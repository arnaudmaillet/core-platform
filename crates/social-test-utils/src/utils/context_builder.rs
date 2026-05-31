// crates/social-test-utils/src/test_context_builder.rs

use crate::SocialTestContext;
use auth::{TokenValidator, interceptors::AuthInterceptor};
use auth_test_utils::KeycloakTestContext;
use infra_fred::RedisIdempotencyRepository;
use infra_test::TestContextBuilder;
use shared_proto::social::v1::social_service_server::SocialServiceServer;
use social::SocialServiceBuilder;
use social::services::SocialService;
use std::sync::Arc;
use tokio::sync::oneshot;
use tonic::transport::Server;

pub struct SocialTestContextBuilder {
    kernel_builder: TestContextBuilder<()>,
    with_grpc: bool,
    has_kafka: bool,
    mock_validator: Option<Arc<dyn TokenValidator>>,
}

impl SocialTestContextBuilder {
    pub fn new() -> Self {
        Self {
            kernel_builder: TestContextBuilder::new()
                .with_scylla(vec!["crates/social/migrations/scylla"])
                .with_redis(),
            with_grpc: false,
            has_kafka: false,
            mock_validator: None,
        }
    }

    pub fn with_grpc_server(mut self) -> Self {
        self.with_grpc = true;
        self
    }

    pub fn with_kafka(mut self) -> Self {
        self.kernel_builder = self.kernel_builder.with_kafka();
        self.has_kafka = true;
        self
    }

    pub fn with_mock_auth(mut self, validator: Arc<dyn TokenValidator>) -> Self {
        self.mock_validator = Some(validator);
        self
    }

    pub async fn build_e2e(self) -> SocialTestContext {
        tracing::info!("Building Social test infrastructure...");
        let kernel_infra = self.kernel_builder.build().await;

        // Extraction des ressources pour le serveur
        let scylla_session = kernel_infra.scylla().session();
        let redis_repo = kernel_infra.redis().repository();
        let redis_pool = redis_repo.pool().clone();

        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let (ready_tx, ready_rx) = oneshot::channel();

        if self.with_grpc {
            tracing::info!("Starting Social gRPC server...");
            let custom_validator = self.mock_validator.clone();

            tokio::spawn(async move {
                // 💡 CHOIX DU VALIDATEUR : Utilisation du mock s'il est fourni, sinon fallback Docker
                let validator = match custom_validator {
                    Some(mock) => mock,
                    None => {
                        let auth_ctx = KeycloakTestContext::restore(
                            "master",
                            "social-service-test".to_string(),
                        )
                        .await;
                        auth_ctx.validator.clone()
                    }
                };

                let interceptor = AuthInterceptor::new(validator);

                let idempotency_repo = Arc::new(RedisIdempotencyRepository::new(
                    redis_pool.clone(),
                    "social_e2e",
                    300,
                ));
                let builder = SocialServiceBuilder::new(
                    scylla_session,
                    redis_pool,
                    redis_repo,
                    idempotency_repo,
                );

                let app_ctx = builder.build_context().await;
                let bus = builder.build_command_bus();
                let social_svc = SocialService::new(bus, app_ctx);

                let listener = tokio::net::TcpListener::bind("[::1]:0").await.unwrap();
                let actual_addr = listener.local_addr().unwrap();
                let incoming = tokio_stream::wrappers::TcpListenerStream::new(listener);

                tracing::info!(port = %actual_addr.port(), "Social gRPC server listening");
                ready_tx.send(actual_addr).ok();

                Server::builder()
                    .add_service(SocialServiceServer::with_interceptor(
                        social_svc,
                        interceptor,
                    ))
                    .serve_with_incoming_shutdown(incoming, async {
                        shutdown_rx.await.ok();
                    })
                    .await
                    .unwrap();
            });
        }

        let addr = if self.with_grpc {
            Some(ready_rx.await.expect("gRPC server failed to start"))
        } else {
            None
        };

        SocialTestContext::new(kernel_infra, addr, Some(shutdown_tx))
    }
}
