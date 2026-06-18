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
use shared_kernel::command::CommandBus;
use shared_kernel::environment::ClusterContext;
use shared_kernel::messaging::{EventConsumer, EventEnvelope};
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
    cluster_ctx: ClusterContext,
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
            cluster_ctx: ClusterContext::default(),
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

    pub fn with_cluster_ctx(mut self, ctx: ClusterContext) -> Self {
        self.cluster_ctx = ctx;
        self
    }

    pub async fn build_e2e(self) -> ProfileTestContext {
        tracing::info!("Starting E2E infrastructure build (Containers startup)...");

        let kernel_infra = self.kernel_builder.build().await;
        // 1. EXTRACTION ET CLONES POSSÉDÉS (OWNED) DANS LE THREAD PRINCIPAL
        let scylla_session_owned = kernel_infra.scylla().session().clone();
        let fred_cache_owned = (*kernel_infra.redis().cache()).clone();
        let fred_idempotency_owned = (*kernel_infra.redis().idempotency()).clone();

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

        let targets = vec![
            ScyllaTableTarget::new("slugs", 3),
            ScyllaTableTarget::new("profiles_by_account", 6),
        ];

        let scylla_orch = ScyllaOrchestrator::new(
            scylla_session_owned.clone(),
            migration_path,
            targets,
            self.cluster_ctx.region().to_string(),
        );
        infra_orchestrator.add(Box::new(scylla_orch));

        infra_orchestrator
            .run_all()
            .await
            .expect("Database orchestrator failed to stabilize schema");

        let target_keyspace = format!(
            "{}_profile_storage",
            self.cluster_ctx.region().to_string().to_lowercase()
        );

        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let (ready_tx, ready_rx) = oneshot::channel();

        // 2. DISPATCH DES COPIES POSSÉDÉES SELON LE MODE DE SERVICE
        match self.service_mode {
            Some(ServiceMode::Grpc) => {
                let session = scylla_session_owned;
                let cache_repo = fred_cache_owned;
                let idempotency_repo = fred_idempotency_owned;
                let custom_validator = self.mock_validator.clone();
                let keyspace_name = target_keyspace;

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

                    // Utilisation des structures possédées locales au thread
                    let mut command_bus = CommandBus::new(
                        Some(Arc::new(idempotency_repo.clone())),
                        Some(Arc::new(cache_repo)),
                    );

                    let routing_store = Arc::new(
                        ScyllaProfileRoutingStore::new(session.clone())
                            .await
                            .unwrap(),
                    );

                    let profile_store = Arc::new(
                        ScyllaProfileStore::new(session, keyspace_name)
                            .await
                            .unwrap(),
                    );

                    let builder = ProfileServiceBuilder::new(
                        profile_store,
                        routing_store,
                        Arc::new(idempotency_repo),
                        self.cluster_ctx,
                    );

                    let kernel_ctx = builder.build_kernel_ctx();
                    builder.register_handlers(&mut command_bus);

                    let shared_bus = Arc::new(command_bus);
                    let svc = Server::builder()
                        .add_service(ProfileIdentityServiceServer::with_interceptor(
                            ProfileIdentityService::new(shared_bus.clone(), kernel_ctx.clone()),
                            interceptor.clone(),
                        ))
                        .add_service(ProfileMediaServiceServer::with_interceptor(
                            ProfileMediaService::new(shared_bus.clone(), kernel_ctx.clone()),
                            interceptor.clone(),
                        ))
                        .add_service(ProfileMetadataServiceServer::with_interceptor(
                            ProfileMetadataService::new(shared_bus.clone(), kernel_ctx),
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
                let session = scylla_session_owned;
                let cache_repo = fred_cache_owned;
                let idempotency_repo = fred_idempotency_owned;
                let brokers = kafka_brokers.unwrap();
                let keyspace_name = target_keyspace;

                tokio::spawn(async move {
                    let routing_store = Arc::new(
                        ScyllaProfileRoutingStore::new(session.clone())
                            .await
                            .unwrap(),
                    );

                    let profile_store = Arc::new(
                        ScyllaProfileStore::new(session, keyspace_name)
                            .await
                            .unwrap(),
                    );

                    let builder = ProfileServiceBuilder::new(
                        profile_store,
                        routing_store,
                        Arc::new(idempotency_repo.clone()),
                        self.cluster_ctx,
                    );

                    let kernel_ctx = Arc::new(builder.build_kernel_ctx());
                    let command_bus = CommandBus::new(
                        Some(Arc::new(idempotency_repo)),
                        Some(Arc::new(cache_repo)),
                    );

                    let shared_bus = Arc::new(command_bus);
                    let account_consumer = Arc::new(AccountConsumer::new(
                        shared_bus.clone(),
                        (*kernel_ctx).clone(),
                    ));
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
