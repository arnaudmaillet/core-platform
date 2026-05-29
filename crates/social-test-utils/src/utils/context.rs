// crates/social-test-utils/src/test_context.rs

use crate::SocialTestContextBuilder;
use infra_test::TestContext;
use std::net::SocketAddr;
use tokio::sync::oneshot;

pub struct SocialTestContext {
    kernel: TestContext,
    pub server_addr: Option<SocketAddr>,
    shutdown_tx: Option<oneshot::Sender<()>>,
}

impl SocialTestContext {
    pub(crate) fn new(
        kernel: TestContext,
        server_addr: Option<SocketAddr>,
        shutdown_tx: Option<oneshot::Sender<()>>,
    ) -> Self {
        Self {
            kernel,
            server_addr,
            shutdown_tx,
        }
    }

    pub fn builder() -> SocialTestContextBuilder {
        SocialTestContextBuilder::new()
    }

    pub fn kernel(&self) -> &TestContext {
        &self.kernel
    }

    pub fn grpc_url(&self) -> String {
        self.server_addr
            .map(|addr| format!("http://{}", addr))
            .expect("gRPC server address not set. Did you call .with_grpc_server()?")
    }

    pub async fn shutdown(self) {
        if let Some(tx) = self.shutdown_tx {
            let _ = tx.send(());
        }
        self.kernel.shutdown().await;
    }
}
