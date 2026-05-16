// crates/shared-kernel/src/test_utils/test_context.rs

use crate::test_utils::{
    KafkaTestContext, PostgresTestContext, RedisTestContext, ScyllaTestContext, TestContextBuilder
};
use std::net::SocketAddr;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

pub struct TestContext {
    postgres_ctx: Option<PostgresTestContext>,
    redis_ctx: Option<RedisTestContext>,
    scylla_ctx: Option<ScyllaTestContext>,
    kafka_ctx: Option<KafkaTestContext>,

    // Champs optionnels pour le mode E2E
    server_addr: Option<SocketAddr>,
    shutdown_tx: Option<oneshot::Sender<()>>,
    server_handle: Option<JoinHandle<()>>,
}

impl TestContext {
    pub(crate) fn new(
        postgres_ctx: Option<PostgresTestContext>,
        redis_ctx: Option<RedisTestContext>,
        scylla_ctx: Option<ScyllaTestContext>,
        kafka_ctx: Option<KafkaTestContext>,
        server_addr: Option<SocketAddr>,
        shutdown_tx: Option<oneshot::Sender<()>>,
        server_handle: Option<JoinHandle<()>>,
    ) -> Self {
        Self {
            postgres_ctx,
            redis_ctx,
            scylla_ctx,
            kafka_ctx,
            server_addr,
            shutdown_tx,
            server_handle,
        }
    }

    // --- Getters d'infrastructure (avec as_ref().expect() pour la sécurité) ---

    pub fn postgres(&self) -> &PostgresTestContext {
        self.postgres_ctx.as_ref().expect(
            "PostgresContext not initialized. Did you call .with_postgres() in the builder?",
        )
    }

    pub fn redis(&self) -> &RedisTestContext {
        self.redis_ctx
            .as_ref()
            .expect("RedisContext not initialized. Did you call .with_redis() in the builder?")
    }

    pub fn scylla(&self) -> &ScyllaTestContext {
        self.scylla_ctx
            .as_ref()
            .expect("ScyllaContext not initialized. Did you call .with_scylla() in the builder?")
    }

    pub fn kafka(&self) -> &KafkaTestContext {
        self.kafka_ctx
            .as_ref()
            .expect("KafkaContext not initialized. Did you call .with_kafka() in the builder?")
    }

    // --- Getters E2E ---

    pub fn server_addr(&self) -> SocketAddr {
        self.server_addr
            .expect("Server address is not available. Check if you used build_e2e()")
    }

    pub fn grpc_url(&self) -> String {
        format!("http://{}", self.server_addr())
    }

    pub fn builder() -> TestContextBuilder<()> {
        TestContextBuilder::new()
    }

    /// Arrête proprement toutes les ressources
    pub async fn shutdown(mut self) {
        // 1. Arrêt du serveur gRPC
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        if let Some(handle) = self.server_handle.take() {
            let _ = handle.await;
        }

        // 2. Les containers Docker sont dropés ici.
        drop(self.postgres_ctx);
        drop(self.redis_ctx);
        drop(self.scylla_ctx);
        drop(self.kafka_ctx);
    }
}
