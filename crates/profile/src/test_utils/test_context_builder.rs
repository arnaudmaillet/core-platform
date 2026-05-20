// crates/profile/src/test_utils/test_context_builder.rs

use crate::ProfileServiceBuilder;
use crate::kafka::AccountConsumer;
use crate::services::{ProfileIdentityService, ProfileMediaService, ProfileMetadataService};
use crate::test_utils::ProfileTestContext;
use auth::{AuthInterceptor, KeycloakTestContext, KeycloakValidator};
use shared_kernel::kafka::KafkaEventConsumer;
use shared_kernel::messaging::{EventConsumer, EventEnvelope};
use shared_kernel::test_utils::TestContextBuilder;
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
}

impl ProfileTestContextBuilder {
    pub fn new() -> Self {
        let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let pg_migrations = manifest_dir.join("migrations/postgres");

        Self {
            kernel_builder: TestContextBuilder::new()
                .with_postgres(vec![pg_migrations])
                .with_redis(),
            service_mode: None,
            has_kafka: false,
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

    pub async fn build_e2e(self) -> ProfileTestContext {
        tracing::info!("Starting E2E infrastructure build...");

        let kernel_infra = self.kernel_builder.build().await;
        let pg_pool = kernel_infra.postgres().pool().clone();
        let redis_repo = kernel_infra.redis().repository();
        let kafka_brokers = self
            .has_kafka
            .then(|| kernel_infra.kafka().bootstrap_servers().to_string());

        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let (ready_tx, ready_rx) = oneshot::channel();

        match self.service_mode {
            Some(ServiceMode::Grpc) => {
                tracing::info!("Mode selected: gRPC Server");
                let pg = pg_pool.clone();
                let redis = redis_repo.clone();

                tokio::spawn(async move {
                    tracing::debug!("gRPC server task spawning...");
                    let auth_ctx = KeycloakTestContext::restore("master").await;
                    let validator = Arc::new(
                        KeycloakValidator::new(&auth_ctx.uri, &auth_ctx.realm)
                            .await
                            .unwrap(),
                    );
                    let interceptor = AuthInterceptor::new(validator);

                    let builder = ProfileServiceBuilder::new(pg, redis);
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
                    let listener = tokio::net::TcpListener::bind(addr)
                        .await
                        .expect("Failed to bind gRPC port");
                    let actual_addr = listener.local_addr().unwrap();
                    let incoming = tokio_stream::wrappers::TcpListenerStream::new(listener);

                    tracing::info!(port = %actual_addr.port(), "gRPC server listening");
                    ready_tx.send(actual_addr).ok();

                    svc.serve_with_incoming_shutdown(incoming, async {
                        shutdown_rx.await.ok();
                        tracing::info!("gRPC server shutting down");
                    })
                    .await
                    .unwrap();
                });
            }
            Some(ServiceMode::KafkaWorker) => {
                tracing::info!("Mode selected: Kafka Worker");
                let pg = pg_pool.clone();
                let redis = redis_repo.clone();
                let brokers = kafka_brokers.unwrap();

                tokio::spawn(async move {
                    tracing::debug!("Kafka worker task spawning...");
                    let builder = ProfileServiceBuilder::new(pg, redis);
                    let app_ctx = builder.build_context();
                    let bus = builder.build_command_bus();

                    let account_consumer =
                        Arc::new(AccountConsumer::new(bus.clone(), (*app_ctx).clone()));
                    let kafka_transport =
                        KafkaEventConsumer::new(&brokers, "profile-worker-test-group", 10);

                    let handler = Box::new(move |envelope: EventEnvelope| {
                        let consumer = Arc::clone(&account_consumer);
                        let fut: std::pin::Pin<
                            Box<
                                dyn std::future::Future<Output = shared_kernel::core::Result<()>>
                                    + Send,
                            >,
                        > = Box::pin(async move {
                            let raw = serde_json::to_vec(&envelope.payload).unwrap();
                            consumer
                                .on_message_received(&raw)
                                .await
                                .map_err(|e| shared_kernel::core::Error::internal(e.to_string()))
                        });
                        fut
                    });

                    tracing::info!("Kafka consumer loop started");
                    ready_tx.send("[::1]:0".parse().unwrap()).ok();

                    if let Err(e) = kafka_transport.consume("account.events", handler).await {
                        tracing::error!(error = %e, "Kafka consumer loop crashed");
                    }
                });
            }
            None => tracing::warn!("No service mode selected for E2E build"),
        }

        let addr = ready_rx
            .await
            .expect("Service failed to start (timeout or crash)");
        tracing::info!(addr = %addr, "E2E infrastructure ready");
        ProfileTestContext::new(kernel_infra, Some(addr), Some(shutdown_tx))
    }
}
