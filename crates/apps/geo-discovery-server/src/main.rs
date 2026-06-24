//! `geo-discovery-server` — the deployable geo-discovery binary.

use std::net::SocketAddr;

use geo_discovery::service::GeoDiscoveryService;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let addr: SocketAddr = std::env::var("GEO_GRPC_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:50054".to_owned())
        .parse()?;

    service_runtime::serve::<GeoDiscoveryService>(addr).await
}
