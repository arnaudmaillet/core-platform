//! `realtime-gateway` — the deployable binary for the realtime **edge plane**.
//!
//! Runs two listeners: the internal gRPC health/reflection plane via
//! [`service_runtime::serve`] on :50066, while [`RealtimeGatewayService::build`]
//! spawns the public WSS server (default :8443) and the node-channel subscriber as
//! background tasks. Scaled — and autoscaled on connection-count + memory, NOT
//! CPU — independently of `realtime-dispatcher`.

use realtime::RealtimeGatewayService;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let addr = std::env::var("REALTIME_GATEWAY_GRPC_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:50066".to_owned())
        .parse()?;
    service_runtime::serve::<RealtimeGatewayService>(addr).await
}
