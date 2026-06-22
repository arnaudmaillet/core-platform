//! Phase 1 structural smoke check for the consumer-runtime test harness.
//!
//! This file exists so the harness is compiled as part of a real test target and its full
//! public surface is type-checked. It is `#[ignore]`d because it needs a Docker daemon to
//! boot the broker; the actual scenarios (A–K) arrive in later phases. Compile it without
//! running via: `cargo test -p transport --features integration-kafka --no-run`.
#![cfg(feature = "integration-kafka")]

mod harness;

use std::time::Duration;

use harness::{TestContext, await_until};

#[tokio::test]
#[ignore = "requires Docker; Phase 1 only verifies the harness compiles and wires up"]
async fn harness_plumbing_is_wired() {
    let ctx = TestContext::new().await;

    // Every factory and observability helper is touched once, so their signatures are
    // verified at compile time even though the body never runs in CI without a broker.
    let _producer = ctx.producer();
    let _broken_producer = ctx.broken_producer();
    let _consumer = ctx.consumer();
    let _dlq_consumer = ctx.dlq_consumer();

    let _committed = ctx.committed_offset(0).await;
    let _dlq_record = ctx.next_dlq_record(Duration::from_millis(500)).await;

    let _polled = await_until(
        Duration::from_secs(1),
        Duration::from_millis(50),
        || async { Some(()) },
    )
    .await;
}
