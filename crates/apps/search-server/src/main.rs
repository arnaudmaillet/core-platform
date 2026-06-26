//! `search-server` — the deployable search binary. A one-liner over the shared
//! fleet runtime: telemetry, externalized config + hot-reload, ingress layers,
//! dynamic health, and graceful shutdown are all the runtime's job. The ingestion
//! consumers self-spawn inside `SearchService::build`.

use std::net::SocketAddr;

use search::service::SearchService;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let addr: SocketAddr = std::env::var("SEARCH_GRPC_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:50062".to_owned())
        .parse()?;

    service_runtime::serve::<SearchService>(addr).await
}
