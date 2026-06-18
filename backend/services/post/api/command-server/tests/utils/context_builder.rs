use auth::{TokenValidator, interceptors::AuthInterceptor};
use auth_test_utils::KeycloakTestContext;
use infra_test::TestContextBuilder;
use post_assembly::PostCommandAssembly;
use post_command_server::PostCommandService;
use post_proto_bridge::v1::post_command_service_server::PostCommandServiceServer;

use shared_kernel::command::CommandBus;
use shared_kernel::environment::ClusterContext;
use std::sync::Arc;
use tokio::sync::oneshot;
use tonic::transport::Server;

use crate::utils::context::PostTestContext;

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

        // On lève les vrais conteneurs de test techniques (ScyllaDB et Redis éphémères)
        let kernel_infra = self.kernel_builder.build().await;
        let scylla_session_owned = kernel_infra.scylla().session().clone();
        let scylla_keyspace = kernel_infra.scylla().keyspace().to_string();

        // Récupération de l'implémentation concrète de ton CacheRepository pour le test
        let fred_cache_owned = (*kernel_infra.redis().cache()).clone();

        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let (ready_tx, ready_rx) = oneshot::channel();

        if self.with_grpc {
            tracing::info!("Starting Post gRPC server for End-to-End testing...");
            let custom_validator = self.mock_validator.clone();
            let cluster_ctx = self.cluster_ctx;

            let cache_repo = fred_cache_owned;
            let keyspace_name = scylla_keyspace;

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

                // En E2E, le CommandBus utilise les vrais composants (ou None si géré en local)
                let command_bus = CommandBus::new(None, None);

                // 4. Passage sur le bootstrap exclusif de Commande
                let container = PostCommandAssembly::bootstrap(
                    scylla_session_owned,
                    cache_repo,
                    keyspace_name,
                    cluster_ctx,
                    command_bus,
                )
                .await
                .expect("Failed to bootstrap PostCommandAssembly during test boot");

                // Instanciation du service spécifique aux mutations d'écriture
                let post_cmd_svc = PostCommandService::new(container);

                let listener = tokio::net::TcpListener::bind("[::1]:0").await.unwrap();
                let actual_addr = listener.local_addr().unwrap();
                let incoming = tokio_stream::wrappers::TcpListenerStream::new(listener);

                tracing::info!(port = %actual_addr.port(), "Post gRPC test server successfully listening");
                ready_tx.send(actual_addr).ok();

                // 5. Enregistrement sur le serveur de commandes exclusif
                Server::builder()
                    .add_service(PostCommandServiceServer::with_interceptor(
                        post_cmd_svc,
                        interceptor,
                    ))
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
