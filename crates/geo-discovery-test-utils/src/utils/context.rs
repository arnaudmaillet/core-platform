// crates/geo-discovery-test-utils/src/test_context.rs

use infra_test::TestContext;
use std::net::SocketAddr;
use tokio::sync::oneshot;

use crate::GeoDiscoveryTestContextBuilder;

pub struct GeoDiscoveryTestContext {
    kernel: TestContext,
    pub server_addr: Option<SocketAddr>,
    shutdown_tx: Option<oneshot::Sender<()>>,
}

impl GeoDiscoveryTestContext {
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

    pub fn builder() -> GeoDiscoveryTestContextBuilder {
        GeoDiscoveryTestContextBuilder::new()
    }

    pub fn kernel(&self) -> &TestContext {
        &self.kernel
    }

    pub fn grpc_url(&self) -> String {
        self.server_addr
            .map(|addr| format!("http://{}", addr))
            .expect("gRPC server address not set. Did you forget to specify .with_grpc_server()?")
    }

    pub async fn shutdown(self) {
        if let Some(tx) = self.shutdown_tx {
            let _ = tx.send(());
        }
        self.kernel.shutdown().await;
    }
}