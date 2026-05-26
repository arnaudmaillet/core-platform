// crates/post-test-utils/src/test_context_builder.rs

use crate::resolvers::ProfileResolverStub;
use crate::utils::PostTestContext;
use auth::{AuthInterceptor, KeycloakValidator};
use auth_test_utils::KeycloakTestContext;
use infra_fred::RedisIdempotencyRepository;
use infra_test::TestContextBuilder;
use post::PostServiceBuilder;
use post::services::PostService;
use shared_proto::post::v1::post_service_server::PostServiceServer;
use std::sync::Arc;
use tokio::sync::oneshot;
use tonic::transport::Server;

pub struct PostTestContextBuilder {
    kernel_builder: TestContextBuilder<()>,
    with_grpc: bool,
    migrations_paths: Vec<String>,
}

impl PostTestContextBuilder {
    pub fn new() -> Self {
        Self {
            kernel_builder: TestContextBuilder::new().with_redis(),
            with_grpc: false,
            migrations_paths: vec!["crates/post/migrations/scylla".to_string()],
        }
    }

    /// Permet de surcharger dynamiquement le chemin des migrations CQL si nécessaire depuis le test
    pub fn with_migrations(mut self, paths: &[&str]) -> Self {
        self.migrations_paths = paths.iter().map(|s| s.to_string()).collect();
        self
    }

    pub fn with_grpc_server(mut self) -> Self {
        self.with_grpc = true;
        self
    }

    pub async fn build_e2e(mut self) -> PostTestContext {
        tracing::info!("Building Post test infrastructure...");

        // Configuration des chemins de migrations au kernel builder avant de build
        let paths_refs: Vec<&str> = self.migrations_paths.iter().map(|s| s.as_str()).collect();
        self.kernel_builder = self.kernel_builder.with_scylla(paths_refs);

        let kernel_infra = self.kernel_builder.build().await;

        // Extraction des instances d'infra éphémères du conteneur de test
        let scylla_session = kernel_infra.scylla().session();
        let scylla_keyspace = kernel_infra.scylla().keyspace().to_string();
        let redis_repo = kernel_infra.redis().repository();
        let redis_pool = redis_repo.pool().clone();
        let profile_resolver = Arc::new(ProfileResolverStub::default());

        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let (ready_tx, ready_rx) = oneshot::channel();

        if self.with_grpc {
            tracing::info!("Starting Post gRPC server for End-to-End testing...");
            tokio::spawn(async move {
                let auth_ctx = KeycloakTestContext::restore("master").await;
                let validator = Arc::new(
                    KeycloakValidator::new(&auth_ctx.uri, &auth_ctx.realm)
                        .await
                        .unwrap(),
                );
                let interceptor = AuthInterceptor::new(validator);

                // Isolation du dépôt d'idempotence pour l'E2E
                let idempotency_repo = Arc::new(RedisIdempotencyRepository::new(
                    redis_pool.clone(),
                    "post_e2e",
                    300,
                ));

                // On utilise ton PostServiceBuilder d'application
                let builder = PostServiceBuilder::new(
                    scylla_keyspace,
                    scylla_session,
                    redis_repo,
                    idempotency_repo,
                    profile_resolver.clone(),
                );

                let app_ctx = builder.build_context().await.unwrap();
                let bus = builder.build_command_bus();
                let post_svc = PostService::new(bus, app_ctx);

                // Port 0 force le système d'exploitation à allouer un port TCP éphémère libre
                let listener = tokio::net::TcpListener::bind("[::1]:0").await.unwrap();
                let actual_addr = listener.local_addr().unwrap();
                let incoming = tokio_stream::wrappers::TcpListenerStream::new(listener);

                tracing::info!(port = %actual_addr.port(), "Post gRPC test server successfully listening");
                ready_tx.send(actual_addr).ok();

                Server::builder()
                    .add_service(PostServiceServer::with_interceptor(post_svc, interceptor))
                    .serve_with_incoming_shutdown(incoming, async {
                        let _ = shutdown_rx.await;
                    })
                    .await
                    .unwrap();
            });
        }

        let addr = if self.with_grpc {
            Some(
                ready_rx
                    .await
                    .expect("Post gRPC test server failed to start"),
            )
        } else {
            None
        };

        PostTestContext::new(kernel_infra, addr, Some(shutdown_tx))
    }
}
