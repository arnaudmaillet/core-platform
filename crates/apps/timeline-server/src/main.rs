//! `timeline-server` — the deployable timeline binary.

use std::net::SocketAddr;

use timeline::service::TimelineService;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let addr: SocketAddr = std::env::var("TIMELINE_GRPC_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:50060".to_owned())
        .parse()?;

    service_runtime::serve::<TimelineService>(addr).await
}
