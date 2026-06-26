//! `counter-worker` — the deployable binary for the counter-analytics **write
//! path**: the heavy stream processor that absorbs the firehose, aggregates it,
//! flushes it durably, and publishes the popularity signal.
//!
//! It runs on the same fleet runtime as the read server but as a consumer-only
//! service: it serves no domain RPC — only the gRPC health + reflection endpoints
//! on its own port (default :50065) so Kubernetes can probe readiness — while the
//! supervised consumers and the drain/flush loop do the work. Scaled independently
//! of `counter-server`.

use counter::CounterWorkerService;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let addr = std::env::var("COUNTER_WORKER_GRPC_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:50065".to_owned())
        .parse()?;
    service_runtime::serve::<CounterWorkerService>(addr).await
}
