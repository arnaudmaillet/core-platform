//! `auth-server` — the deployable auth binary. A one-liner over the shared fleet
//! runtime: telemetry, externalized config + hot-reload, ingress layers, dynamic
//! health, and graceful shutdown are all the runtime's job.

use std::net::SocketAddr;

use auth::service::AuthService;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let addr: SocketAddr = std::env::var("AUTH_GRPC_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:50060".to_owned())
        .parse()?;

    service_runtime::serve::<AuthService>(addr).await
}
