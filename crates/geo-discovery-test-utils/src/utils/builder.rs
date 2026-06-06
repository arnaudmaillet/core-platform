// crates/geo_discovery-test-utils/src/test_context_builder.rs

use auth::{TokenValidator, interceptors::AuthInterceptor};
use auth_test_utils::TokenValidatorStub;
use geo_discovery::db::FredMapCacheRepository;
use geo_discovery::services::GeoDiscoveryService;
use geo_discovery::{db::ScyllaMapPersistenceRepository, handlers::HydrateTileCacheHandler};
use infra_fred::RedisIdempotencyRepository;
use infra_scylla::scylla::client::session::Session;
use infra_test::TestContextBuilder;
use shared_kernel::types::Region;
use shared_proto::geo_discovery::v1::geo_discovery_service_server::GeoDiscoveryServiceServer;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};
use tonic::transport::Server;

use crate::GeoDiscoveryTestContext;
use crate::resolvers::EngagementResolverStub;
use geo_discovery::context::GeoDiscoveryAppContext;

pub struct GeoDiscoveryTestContextBuilder {
    kernel_builder: TestContextBuilder<()>,
    with_grpc: bool,
    migrations_paths: Vec<String>,
    mock_validator: Option<Arc<dyn TokenValidator>>,
}

impl GeoDiscoveryTestContextBuilder {
    pub fn new() -> Self {
        Self {
            kernel_builder: TestContextBuilder::new().with_redis(),
            with_grpc: false,
            migrations_paths: vec!["crates/geo_discovery/migrations/scylla".to_string()],
            mock_validator: None,
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

    pub async fn build_e2e(mut self) -> GeoDiscoveryTestContext {
        tracing::info!("Building GeoDiscovery Real-Infra test container...");

        let paths_refs: Vec<&str> = self.migrations_paths.iter().map(|s| s.as_str()).collect();
        self.kernel_builder = self.kernel_builder.with_scylla(paths_refs);

        let kernel_infra = self.kernel_builder.build().await;

        let scylla_session: Arc<Session> = kernel_infra.scylla().session();
        let redis_repo = kernel_infra.redis().repository();
        let redis_pool = redis_repo.pool().clone();

        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let (ready_tx, ready_rx) = oneshot::channel();

        if self.with_grpc {
            tracing::info!("Starting GeoDiscovery gRPC test server...");

            let validator = match self.mock_validator.clone() {
                Some(explicit_validator) => explicit_validator,
                None => Arc::new(TokenValidatorStub::new()),
            };

            let interceptor = AuthInterceptor::new(validator);

            let idempotency_repo = Arc::new(RedisIdempotencyRepository::new(
                redis_pool.clone(),
                "geo_discovery_e2e",
                300,
            ));

            let persistence_repo = Arc::new(
                ScyllaMapPersistenceRepository::new(scylla_session.clone())
                    .await
                    .unwrap(),
            );

            let cache_repo = Arc::new(FredMapCacheRepository::new(redis_pool.clone()));
            let engagement_resolver = Arc::new(EngagementResolverStub);

            let max_posts_per_tile = 50;
            let (hydration_sender, mut hydration_receiver) = mpsc::channel(100);

            let app_ctx = GeoDiscoveryAppContext::new(
                persistence_repo.clone(),
                cache_repo.clone(),
                idempotency_repo,
                engagement_resolver,
                hydration_sender,
            );

            let hydration_handler = Arc::new(HydrateTileCacheHandler::new(
                cache_repo,
                persistence_repo,
                max_posts_per_tile,
            ));

            tokio::spawn(async move {
                while let Some(cmd) = hydration_receiver.recv().await {
                    let _ = hydration_handler.handle(cmd).await;
                }
            });

            let query_ctx = app_ctx.query(Region::default());
            let geo_grpc_svc = GeoDiscoveryService::new(query_ctx);

            let listener = tokio::net::TcpListener::bind("[::1]:0").await.unwrap();
            let actual_addr = listener.local_addr().unwrap();
            let incoming = tokio_stream::wrappers::TcpListenerStream::new(listener);

            tracing::info!(port = %actual_addr.port(), "GeoDiscovery gRPC test server listening");
            ready_tx.send(actual_addr).ok();

            Server::builder()
                .add_service(GeoDiscoveryServiceServer::with_interceptor(
                    geo_grpc_svc,
                    interceptor,
                ))
                .serve_with_incoming_shutdown(incoming, async {
                    let _ = shutdown_rx.await;
                })
                .await
                .unwrap();
        }

        let addr = if self.with_grpc {
            Some(
                ready_rx
                    .await
                    .expect("GeoDiscovery gRPC test server failed to start"),
            )
        } else {
            None
        };

        GeoDiscoveryTestContext::new(kernel_infra, addr, Some(shutdown_tx))
    }
}
