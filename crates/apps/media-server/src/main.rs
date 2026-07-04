//! `media-server` — the deployable media binary. A one-liner over the shared fleet
//! runtime: telemetry, externalized config + hot-reload, ingress layers, dynamic
//! health, and graceful shutdown are all the runtime's job. The Plane B processing
//! consumer and the moderation takedown consumer self-spawn inside
//! `MediaService::build`. The control plane carries no bytes (see `media-api`).

use std::net::SocketAddr;

use media::service::MediaService;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let addr: SocketAddr = std::env::var("MEDIA_GRPC_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:50063".to_owned())
        .parse()?;

    service_runtime::serve::<MediaService>(addr).await
}
