//! `realtime-dispatcher` — the deployable binary for the realtime **fan-out path**.
//!
//! Serves only the gRPC health/reflection plane via [`service_runtime::serve`] on
//! :50067, while [`RealtimeDispatcherService::build`] spawns the supervised
//! `run_consumer` loops that decode upstream events and fan them out to the owning
//! gateway nodes. Scaled independently of `realtime-gateway`.

use realtime::RealtimeDispatcherService;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let addr = std::env::var("REALTIME_DISPATCHER_GRPC_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:50067".to_owned())
        .parse()?;
    service_runtime::serve::<RealtimeDispatcherService>(addr).await
}
