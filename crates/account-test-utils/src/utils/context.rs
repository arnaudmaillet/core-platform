// crates/account/src/test_utils/test_context.rs

use crate::AccountTestContextBuilder;
use infra_sqlx::sqlx::PgPool;
use infra_test::TestContext;
use std::net::SocketAddr;
use tokio::sync::oneshot;

pub struct AccountTestContext {
    kernel_context: TestContext,
    pub server_addr: Option<SocketAddr>,
    shutdown_tx: Option<oneshot::Sender<()>>,
}

impl AccountTestContext {
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

    pub fn builder() -> AccountTestContextBuilder {
        AccountTestContextBuilder::new()
    }

    pub fn kernel(&self) -> &TestContext {
        &self.kernel_context
    }

    pub fn pg_pool(&self) -> PgPool {
        self.kernel_context.postgres().pool().clone()
    }

    pub fn grpc_url(&self) -> String {
        self.server_addr
            .map(|addr| format!("http://{}", addr))
            .expect("gRPC server address not set. Did you call .with_grpc_server()?")
    }

    pub async fn shutdown(self) {
        // Envoi du signal de shutdown si le serveur a été lancé
        if let Some(tx) = self.shutdown_tx {
            let _ = tx.send(());
        }
        self.kernel_context.shutdown().await;
    }
}
