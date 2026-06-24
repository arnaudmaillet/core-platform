//! `chat-server` — the deployable chat binary.
//!
//! Intentionally trivial: all bootstrap (telemetry, config + hot-reload, gRPC
//! serving, graceful shutdown) lives in [`service_runtime`], and all domain
//! wiring lives in the `chat` library. Adding a deployable to the fleet is a new
//! crate this size.

use std::net::SocketAddr;

use chat::service::ChatService;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let addr: SocketAddr = std::env::var("CHAT_GRPC_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:50051".to_owned())
        .parse()?;

    service_runtime::serve::<ChatService>(addr).await
}
