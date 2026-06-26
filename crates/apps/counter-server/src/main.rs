//! `counter-server` — the deployable binary for the counter-analytics **read
//! path**: a low-latency, stateless gRPC server over the hot counter tier.
//!
//! gRPC port **50064** (auth 50060 · moderation 50061 · search 50062 · media
//! 50063 → next free). This process serves reads ONLY; the firehose aggregation
//! lives in the separate `counter-worker` binary.

use counter::CounterReadService;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let addr = std::env::var("COUNTER_GRPC_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:50064".to_owned())
        .parse()?;
    service_runtime::serve::<CounterReadService>(addr).await
}
