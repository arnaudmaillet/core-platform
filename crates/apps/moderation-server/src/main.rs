//! `moderation-server` — the deployable moderation binary. A one-liner over the
//! shared fleet runtime: telemetry, externalized config + hot-reload, ingress
//! layers, dynamic health, and graceful shutdown are all the runtime's job. The
//! Plane A ingestion consumers self-spawn inside `ModerationService::build`.

use std::net::SocketAddr;

use moderation::service::ModerationService;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let addr: SocketAddr = std::env::var("MODERATION_GRPC_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:50061".to_owned())
        .parse()?;

    service_runtime::serve::<ModerationService>(addr).await
}
