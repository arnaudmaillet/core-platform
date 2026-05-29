use crate::ProfileTestContextBuilder;
use infra_sqlx::sqlx::PgPool;
use infra_test::TestContext;
use std::net::SocketAddr;
use tokio::sync::oneshot;

pub struct ProfileTestContext {
    kernel_context: TestContext,
    pub server_addr: Option<SocketAddr>,
    shutdown_tx: Option<oneshot::Sender<()>>,
}

impl ProfileTestContext {
    pub fn builder() -> ProfileTestContextBuilder {
        ProfileTestContextBuilder::new()
    }

    pub fn kernel(&self) -> &TestContext {
        &self.kernel_context
    }

    pub fn pg_pool(&self) -> PgPool {
        self.kernel_context.postgres().pool().clone()
    }

    pub async fn shutdown(self) {
        if let Some(tx) = self.shutdown_tx {
            let _ = tx.send(());
        }
        self.kernel_context.shutdown().await;
    }

    pub fn grpc_url(&self) -> String {
        format!("http://{}", self.server_addr.expect("Server not started"))
    }

    /// Constructeur interne utilisé par le builder
    pub(crate) fn new(
        kernel_context: TestContext,
        server_addr: Option<SocketAddr>,
        shutdown_tx: Option<oneshot::Sender<()>>,
    ) -> Self {
        Self {
            kernel_context,
            server_addr,
            shutdown_tx,
        }
    }
}
