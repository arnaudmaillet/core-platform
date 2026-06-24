//! `profile-server` — the deployable profile binary.
//!
//! Like `chat-server`, intentionally trivial: bootstrap lives in
//! [`service_runtime`] and domain wiring in the `profile` library. Profile's
//! inbound account-event consumer is self-spawned during `ProfileService::build`,
//! so this entrypoint stays a one-liner.

use std::net::SocketAddr;

use profile::service::ProfileService;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let addr: SocketAddr = std::env::var("PROFILE_GRPC_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:50052".to_owned())
        .parse()?;

    service_runtime::serve::<ProfileService>(addr).await
}
