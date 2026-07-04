//! Live-Redis integration: the real Lua claim enforces a global budget end to end.
//!
//! Gated behind `integration-traffic-redis` (needs a Docker daemon); the hermetic unit
//! suite in `tests/lease.rs` covers the algorithm against an in-memory claim source.
//! Run with: `cargo test -p traffic-redis --features integration-traffic-redis`.
#![cfg(feature = "integration-traffic-redis")]

use std::time::{SystemTime, UNIX_EPOCH};

use redis_storage::{RedisClientBuilder, RedisConfig};
use traffic::{Quota, QuotaBackend, TrafficDecision};
use traffic_redis::RedisLeaseBackend;

/// Unique key per run so a rerun within the same window doesn't see leftover budget.
fn unique_key() -> String {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    format!("it:{nanos}")
}

#[tokio::test]
async fn enforces_global_budget_against_real_redis() {
    let endpoint = test_support::containers::redis_endpoint().await;
    let client = RedisClientBuilder::new(RedisConfig {
        hosts: vec![endpoint],
        ..RedisConfig::default()
    })
    .build()
    .await
    .expect("connect redis");

    let backend = RedisLeaseBackend::new(client);
    // budget = ceil(rps * lease_ms / 1000) = 10, in a 10s window long enough that all 20
    // checks fall inside it; burst=1 means every claim is exact (no local over-lease).
    let quota = Quota { rps: 1, burst: 1, lease_ms: 10_000 };
    let key = unique_key();

    let mut allows = 0;
    for _ in 0..20 {
        if matches!(backend.check(&key, quota).await.unwrap(), TrafficDecision::Allow) {
            allows += 1;
        }
    }
    assert_eq!(allows, 10, "the global per-window budget is enforced via Redis");
}
