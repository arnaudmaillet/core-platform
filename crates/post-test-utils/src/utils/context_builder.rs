// crates/post-test-utils/src/test_context_builder.rs

use crate::resolvers::ProfileResolverStub;
use crate::utils::PostTestContext;
use auth::{TokenValidator, interceptors::AuthInterceptor};
use auth_test_utils::KeycloakTestContext;
use infra_test::TestContextBuilder;
use post::PostServiceBuilder;
use post::ScyllaPostRepository;
use post::services::PostService;
use shared_kernel::command::CommandBus;
use shared_kernel::environment::ClusterContext;
use shared_proto::post::v1::post_service_server::PostServiceServer;
use std::sync::Arc;
use tokio::sync::oneshot;
use tonic::transport::Server;

pub struct PostTestContextBuilder {
    kernel_builder: TestContextBuilder<()>,
    with_grpc: bool,
    migrations_paths: Vec<String>,
    mock_validator: Option<Arc<dyn TokenValidator>>,
    cluster_ctx: ClusterContext,
}

impl PostTestContextBuilder {
    pub fn new() -> Self {
        Self {
            kernel_builder: TestContextBuilder::new().with_redis(),
            with_grpc: false,
            migrations_paths: vec!["crates/post/migrations/scylla".to_string()],
            mock_validator: None,
            cluster_ctx: ClusterContext::default(),
        }
    }

    pub fn with_migrations(mut self, paths: &[&str]) -> Self {
        self.migrations_paths = paths.iter().map(|s| s.to_string()).collect();
        self
    }

    pub fn with_grpc_server(mut self) -> Self {
        self.with_grpc = true;
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

    pub async fn build_e2e(mut self) -> PostTestContext {
        tracing::info!("Building Post test infrastructure...");

        let paths_refs: Vec<&str> = self.migrations_paths.iter().map(|s| s.as_str()).collect();
        self.kernel_builder = self.kernel_builder.with_scylla(paths_refs);

        let kernel_infra = self.kernel_builder.build().await;
        let scylla_session_owned = kernel_infra.scylla().session().clone();
        let scylla_keyspace = kernel_infra.scylla().keyspace().to_string();

        let fred_cache_owned = (*kernel_infra.redis().cache()).clone();
        // Note : On n'extrait même pas idempotency_repo ici car le bus des posts n'en a pas besoin !

        let profile_resolver = Arc::new(ProfileResolverStub::default());

        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let (ready_tx, ready_rx) = oneshot::channel();

        if self.with_grpc {
            tracing::info!("Starting Post gRPC server for End-to-End testing...");
            let custom_validator = self.mock_validator.clone();
            let cluster_ctx = self.cluster_ctx;
            let resolver = profile_resolver.clone();

            // Préparation des variables possédées pour le move
            let cache_for_bus = fred_cache_owned;
            let session_for_repo = scylla_session_owned;

            tokio::spawn(async move {
                let validator = match custom_validator {
                    Some(mock) => mock,
                    None => {
                        let auth_ctx =
                            KeycloakTestContext::restore("master", "post-service-test".to_string())
                                .await;
                        auth_ctx.validator.clone()
                    }
                };

                let interceptor = AuthInterceptor::new(validator);
                let mut command_bus = CommandBus::new(None, Some(Arc::new(cache_for_bus)));
                let real_post_repo = Arc::new(
                    ScyllaPostRepository::new(session_for_repo, scylla_keyspace)
                        .await
                        .expect("Failed to initialize real ScyllaPostRepository during test boot"),
                );

                let builder = PostServiceBuilder::new(real_post_repo, resolver, cluster_ctx);

                let kernel_ctx = builder.build_kernel_ctx();
                builder.register_handlers(&mut command_bus);

                let post_svc = PostService::new(command_bus, kernel_ctx);

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
