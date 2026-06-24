//! `engagement-server` — the deployable engagement binary.

use std::net::SocketAddr;

use engagement::service::EngagementService;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let addr: SocketAddr = std::env::var("ENGAGEMENT_GRPC_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:50058".to_owned())
        .parse()?;

    service_runtime::serve::<EngagementService>(addr).await
}
