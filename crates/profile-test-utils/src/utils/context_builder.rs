// crates/profile/src/test_utils/test_context_builder.rs

use crate::ProfileTestContext;
use auth::{TokenValidator, interceptors::AuthInterceptor};
use auth_test_utils::KeycloakTestContext;
use infra_kafka::KafkaEventConsumer;
use infra_test::{
    InfrastructureOrchestrator, ScyllaOrchestrator, ScyllaTableTarget, TestContextBuilder,
};
use profile::ProfileServiceBuilder;
use profile::kafka::AccountConsumer;
use profile::services::{ProfileIdentityService, ProfileMediaService, ProfileMetadataService};
use profile::stores::{ScyllaProfileRoutingStore, ScyllaProfileStore};
use shared_kernel::messaging::{EventConsumer, EventEnvelope};
use shared_kernel::types::Region;
use shared_proto::profile::v1::profile_identity_service_server::ProfileIdentityServiceServer;
use shared_proto::profile::v1::profile_media_service_server::ProfileMediaServiceServer;
use shared_proto::profile::v1::profile_metadata_service_server::ProfileMetadataServiceServer;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::oneshot;
use tonic::transport::Server;

pub enum ServiceMode {
    Grpc,
    KafkaWorker,
}

pub struct ProfileTestContextBuilder {
    kernel_builder: TestContextBuilder<()>,
    service_mode: Option<ServiceMode>,
    has_kafka: bool,
    mock_validator: Option<Arc<dyn TokenValidator>>,
    local_region: Region,
}

impl ProfileTestContextBuilder {
    pub fn new() -> Self {
        Self {
            kernel_builder: TestContextBuilder::new()
                .with_scylla(Vec::<String>::new())
                .with_redis(),
            service_mode: None,
            has_kafka: false,
            mock_validator: None,
            local_region: Region::default(),
        }
    }

    pub fn with_grpc_server(mut self) -> Self {
        self.service_mode = Some(ServiceMode::Grpc);
        self
    }

    pub fn with_kafka_worker(mut self) -> Self {
        self.service_mode = Some(ServiceMode::KafkaWorker);
        self.has_kafka = true;
        self.kernel_builder = self.kernel_builder.with_kafka();
        self
    }

    pub fn with_mock_auth(mut self, validator: Arc<dyn TokenValidator>) -> Self {
        self.mock_validator = Some(validator);
        self
    }

    pub fn with_region(mut self, region: Region) -> Self {
        self.local_region = region;
        self
    }

    pub async fn build_e2e(self) -> ProfileTestContext {
        tracing::info!("Starting E2E infrastructure build (Containers startup)...");

        let kernel_infra = self.kernel_builder.build().await;
        let scylla_session = kernel_infra.scylla().session().clone();
        let redis_repo = kernel_infra.redis().cache();
        let kafka_brokers = self
            .has_kafka
            .then(|| kernel_infra.kafka().bootstrap_servers().to_string());

        let mut infra_orchestrator = InfrastructureOrchestrator::new();

        let mut base_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        if base_path.ends_with("profile-test-utils")
            || base_path.ends_with("profile-command-server")
        {
            base_path.pop();
            base_path.push("profile");
        }
        let migration_path = base_path.join("migrations/scylla");

        // 💡 Tables cibles uniques pour valider les deux zones (Global + Régional)
        let targets = vec![
            ScyllaTableTarget::new("slugs", 3), // Valide global_routing
            ScyllaTableTarget::new("profiles_by_account", 6), // Valide {region}_profile_storage
        ];

        // 💡 On récupère le nom de la région dynamiquement depuis ton build context
        let region_str = self.local_region.to_string();

        let scylla_orch =
            ScyllaOrchestrator::new(scylla_session.clone(), migration_path, targets, region_str);
        infra_orchestrator.add(Box::new(scylla_orch));

        infra_orchestrator
            .run_all()
            .await
            .expect("Database orchestrator failed to stabilize schema");
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let (ready_tx, ready_rx) = oneshot::channel();

        match self.service_mode {
            Some(ServiceMode::Grpc) => {
                let session = scylla_session.clone();
                let redis = redis_repo.clone();
                let custom_validator = self.mock_validator.clone();
                let region = self.local_region;
                let idempotency = kernel_infra.redis().idempotency();

                tokio::spawn(async move {
                    let validator = match custom_validator {
                        Some(mock) => mock,
                        None => {
                            let auth_ctx = KeycloakTestContext::restore(
                                "master",
                                "profile-service-test".to_string(),
                            )
                            .await;
                            auth_ctx.validator.clone()
                        }
                    };

                    let interceptor = AuthInterceptor::new(validator);

                    // Garanti sans crash ni Race Condition désormais
                    let routing_store = Arc::new(
                        ScyllaProfileRoutingStore::new(session.clone())
                            .await
                            .unwrap(),
                    );
                    let profile_store =
                        Arc::new(ScyllaProfileStore::new(session, region).await.unwrap());

                    let builder = ProfileServiceBuilder::new(
                        profile_store,
                        routing_store,
                        redis,
                        idempotency,
                        region,
                    );
                    let app_ctx = builder.build_context();
                    let bus = builder.build_command_bus();

                    let svc = Server::builder()
                        .add_service(ProfileIdentityServiceServer::with_interceptor(
                            ProfileIdentityService::new(bus.clone(), app_ctx.clone()),
                            interceptor.clone(),
                        ))
                        .add_service(ProfileMediaServiceServer::with_interceptor(
                            ProfileMediaService::new(bus.clone(), app_ctx.clone()),
                            interceptor.clone(),
                        ))
                        .add_service(ProfileMetadataServiceServer::with_interceptor(
                            ProfileMetadataService::new(bus, app_ctx),
                            interceptor,
                        ));

                    let addr = "[::1]:0".parse::<SocketAddr>().unwrap();
                    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
                    let actual_addr = listener.local_addr().unwrap();
                    let incoming = tokio_stream::wrappers::TcpListenerStream::new(listener);

                    ready_tx.send(actual_addr).ok();
                    svc.serve_with_incoming_shutdown(incoming, async {
                        shutdown_rx.await.ok();
                    })
                    .await
                    .unwrap();
                });
            }
            Some(ServiceMode::KafkaWorker) => {
                let session = scylla_session.clone();
                let redis = redis_repo.clone();
                let brokers = kafka_brokers.unwrap();
                let region = self.local_region;
                let idempotency = kernel_infra.redis().idempotency();

                tokio::spawn(async move {
                    let routing_store = Arc::new(
                        ScyllaProfileRoutingStore::new(session.clone())
                            .await
                            .unwrap(),
                    );
                    let profile_store =
                        Arc::new(ScyllaProfileStore::new(session, region).await.unwrap());

                    let builder = ProfileServiceBuilder::new(
                        profile_store,
                        routing_store,
                        redis,
                        idempotency,
                        region,
                    );
                    let app_ctx = Arc::new(builder.build_context());
                    let bus = builder.build_command_bus();

                    let account_consumer =
                        Arc::new(AccountConsumer::new(bus.clone(), (*app_ctx).clone()));
                    let kafka_transport =
                        KafkaEventConsumer::new(&brokers, "profile-worker-test-group", 10);

                    let handler = Box::new(move |envelope: EventEnvelope| {
                        let consumer = Arc::clone(&account_consumer);

                        Box::pin(async move {
                            let raw = serde_json::to_vec(&envelope.payload)
                                .map_err(|e| shared_kernel::core::Error::internal(e.to_string()))?;

                            consumer
                                .on_message_received(&raw)
                                .await
                                .map_err(|e| shared_kernel::core::Error::internal(e.to_string()))
                        })
                            as std::pin::Pin<
                                Box<
                                    dyn std::future::Future<
                                            Output = shared_kernel::core::Result<()>,
                                        > + Send,
                                >,
                            >
                    });

                    ready_tx.send("[::1]:0".parse().unwrap()).ok();
                    kafka_transport
                        .consume("account.events", handler)
                        .await
                        .ok();
                });
            }
            None => tracing::warn!("No service mode selected for E2E build"),
        }

        let addr = ready_rx.await.expect("Service failed to start");
        ProfileTestContext::new(kernel_infra, Some(addr), Some(shutdown_tx))
    }
}
