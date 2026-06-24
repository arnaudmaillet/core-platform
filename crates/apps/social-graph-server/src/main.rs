//! `social-graph-server` — the deployable social-graph binary.

use std::net::SocketAddr;

use social_graph::service::SocialGraphService;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let addr: SocketAddr = std::env::var("SOCIAL_GRAPH_GRPC_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:50053".to_owned())
        .parse()?;

    service_runtime::serve::<SocialGraphService>(addr).await
}
