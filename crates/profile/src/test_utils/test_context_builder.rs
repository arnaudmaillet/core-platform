// crates/profile/src/test_utils/test_context_builder.rs

use crate::{test_utils::ProfileTestContext, utils::run_postgres_migrations};
use shared_kernel::test_utils::{TestContext, TestContextBuilder};
use std::future::Future;
use std::net::SocketAddr;
use tokio::sync::oneshot;

pub struct ProfileTestContextBuilder<F = ()> {
    kernel_builder: TestContextBuilder<()>,
    server_factory: Option<F>,
    has_kafka: bool,
}

impl ProfileTestContextBuilder<()> {
    pub fn new() -> Self {
        Self {
            kernel_builder: TestContextBuilder::new().with_postgres().with_redis(),
            server_factory: None,
            has_kafka: false,
        }
    }
}

impl<F> ProfileTestContextBuilder<F> {
    pub fn with_kafka(mut self) -> Self {
        self.kernel_builder = self.kernel_builder.with_kafka();
        self.has_kafka = true;
        self
    }
}

impl ProfileTestContextBuilder<()> {
    pub fn with_server<F, Fut>(self, factory: F) -> ProfileTestContextBuilder<F>
    where
        F: Fn(
                sqlx::PgPool,
                std::sync::Arc<dyn shared_kernel::cache::CacheRepository>,
                Option<String>,
                SocketAddr,
                oneshot::Receiver<()>,
                oneshot::Sender<()>,
            ) -> Fut
            + Send
            + Sync
            + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        ProfileTestContextBuilder {
            kernel_builder: self.kernel_builder,
            server_factory: Some(factory),
            has_kafka: self.has_kafka,
        }
    }
}

/// Extension d'exécution classique (sans serveur gRPC)
impl ProfileTestContextBuilder<()> {
    pub async fn build(self) -> ProfileTestContext {
        let kernel = self.kernel_builder.build().await;
        let ctx = ProfileTestContext::new(kernel);
        run_postgres_migrations(&ctx.kernel().postgres().pool())
            .await
            .expect("Failed to apply profile migrations");
        ctx
    }
}

/// Extension d'exécution E2E (avec serveur gRPC ou Event-Worker)
impl<F, Fut> ProfileTestContextBuilder<F>
where
    F: Fn(
            sqlx::PgPool,
            std::sync::Arc<dyn shared_kernel::cache::CacheRepository>,
            Option<String>,
            SocketAddr,
            oneshot::Receiver<()>,
            oneshot::Sender<()>,
        ) -> Fut
        + Send
        + Sync
        + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    pub async fn build_e2e(self) -> ProfileTestContext {
        let kernel_infra = self.kernel_builder.build().await;

        let pg_pool = kernel_infra.postgres().pool().clone();
        let redis_repo = kernel_infra.redis().repository();

        run_postgres_migrations(&pg_pool)
            .await
            .expect("Failed to apply profile migrations");

        if let Some(factory) = self.server_factory {
            let kafka_bootstrap_servers = if self.has_kafka {
                Some(kernel_infra.kafka().bootstrap_servers().to_string())
            } else {
                None
            };

            let addr: SocketAddr = "[::1]:0".parse().unwrap();
            let listener = tokio::net::TcpListener::bind(addr)
                .await
                .expect("Failed to bind");
            let actual_addr = listener.local_addr().expect("Failed to get local addr");
            drop(listener);

            let (shutdown_tx, shutdown_rx) = oneshot::channel();
            let (ready_tx, ready_rx) = oneshot::channel(); // 💡 Canal de synchronisation déterministe

            // Lancement de la factory utilisateur
            let server_handle = tokio::spawn(async move {
                factory(
                    pg_pool,
                    redis_repo,
                    kafka_bootstrap_servers,
                    actual_addr,
                    shutdown_rx,
                    ready_tx,
                )
                .await;
            });

            ready_rx.await.ok();

            let (pg, redis, scylla, kafka) = kernel_infra.into_parts();

            let final_kernel = TestContext::new(
                pg,
                redis,
                scylla,
                kafka,
                Some(actual_addr),
                Some(shutdown_tx),
                Some(server_handle),
            );

            return ProfileTestContext::new(final_kernel);
        }

        ProfileTestContext::new(kernel_infra)
    }
}
