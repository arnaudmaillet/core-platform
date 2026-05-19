// crates/account/src/test_utils/test_context_builder.rs

use crate::{test_utils::AccountTestContext, utils::run_postgres_migrations};
use shared_kernel::test_utils::{E2EServerStarter, TestContext, TestContextBuilder};
use std::future::Future;
use std::net::SocketAddr;
use tokio::sync::oneshot;

// Un starter interne anonyme qui va encapsuler notre closure utilisateur
struct ClosureServerStarter<F> {
    factory: F,
    pg_pool: sqlx::PgPool,
    redis_repo: std::sync::Arc<dyn shared_kernel::cache::CacheRepository>,
    kafka_bootstrap_servers: Option<String>,
    ready_tx: Option<oneshot::Sender<()>>,
}

#[async_trait::async_trait]
impl<F, Fut> E2EServerStarter for ClosureServerStarter<F>
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
    async fn start_server(&self, addr: SocketAddr, shutdown_rx: oneshot::Receiver<()>) {
        // Cette implémentation est requise par le trait mais inutilisée dans notre flux optimisé en dessous
    }
}

pub struct AccountTestContextBuilder<F = ()> {
    kernel_builder: TestContextBuilder<()>,
    server_factory: Option<F>,
    has_kafka: bool,
}

impl AccountTestContextBuilder<()> {
    pub fn new() -> Self {
        Self {
            kernel_builder: TestContextBuilder::new().with_postgres().with_redis(),
            server_factory: None,
            has_kafka: false,
        }
    }
}

impl<F> AccountTestContextBuilder<F> {
    pub fn with_kafka(mut self) -> Self {
        self.kernel_builder = self.kernel_builder.with_kafka();
        self.has_kafka = true;
        self
    }
}

impl AccountTestContextBuilder<()> {
    pub fn with_server<F, Fut>(self, factory: F) -> AccountTestContextBuilder<F>
    where
        F: Fn(
                sqlx::PgPool,
                std::sync::Arc<dyn shared_kernel::cache::CacheRepository>,
                Option<String>,
                SocketAddr,
                oneshot::Receiver<()>,
                oneshot::Sender<()>, // 💡 Reçoit le canal de synchronisation
            ) -> Fut
            + Send
            + Sync
            + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        AccountTestContextBuilder {
            kernel_builder: self.kernel_builder,
            server_factory: Some(factory),
            has_kafka: self.has_kafka,
        }
    }
}

/// Extension d'exécution classique (sans serveur gRPC)
impl AccountTestContextBuilder<()> {
    pub async fn build(self) -> AccountTestContext {
        let kernel = self.kernel_builder.build().await;
        let ctx = AccountTestContext::new(kernel);

        run_postgres_migrations(&ctx.kernel().postgres().pool())
            .await
            .expect("Failed to apply account migrations");

        ctx
    }
}

/// Extension d'exécution E2E (avec serveur gRPC)
impl<F, Fut> AccountTestContextBuilder<F>
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
    pub async fn build_e2e(self) -> AccountTestContext {
        // Étape 1 : On démarre l'infrastructure brute unifiée (UN SEUL conteneur PostgreSQL)
        let kernel_infra = self.kernel_builder.build().await;

        // Étape 2 : On extrait l'infrastructure pour appliquer les migrations sur CETTE instance
        let pg_pool = kernel_infra.postgres().pool().clone();
        let redis_repo = kernel_infra.redis().repository();

        run_postgres_migrations(&pg_pool)
            .await
            .expect("Failed to apply account migrations");

        // Étape 3 : Si une factory de serveur a été fournie, on démarre Tonic manuellement
        if let Some(factory) = self.server_factory {
            let kafka_bootstrap_servers = if self.has_kafka {
                Some(kernel_infra.kafka().bootstrap_servers().to_string())
            } else {
                None
            };

            // --- Setup Réseau dynamique ---
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

            // L'exécution attend le signal explicite envoyé par le serveur gRPC
            ready_rx.await.ok();

            // Étape 4 : On extrait les composants internes du TestContext d'infrastructure
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

            return AccountTestContext::new(final_kernel);
        }

        AccountTestContext::new(kernel_infra)
    }
}
