//! Lease-algorithm behaviour against an in-memory claim source (no Redis).
//!
//! The fake mirrors the Lua script's semantics and is shared across `LeaseBook`s to model
//! multiple replicas contending for one global budget.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Mutex;

use async_trait::async_trait;
use traffic::{Quota, QuotaError, TrafficDecision};
use traffic_redis::{ClaimSource, LeaseBook};

#[derive(Default)]
struct FakeClaims {
    used: Mutex<HashMap<(String, u64), u64>>,
    calls: AtomicUsize,
    fail: AtomicBool,
}

#[async_trait]
impl ClaimSource for FakeClaims {
    async fn claim(
        &self,
        key: &str,
        window: u64,
        budget: u64,
        want: u64,
        _ttl_ms: u64,
    ) -> Result<u64, QuotaError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.fail.load(Ordering::SeqCst) {
            return Err(QuotaError("backend down".into()));
        }
        let mut used = self.used.lock().unwrap();
        let spent = used.entry((key.to_owned(), window)).or_insert(0);
        if *spent >= budget {
            return Ok(0);
        }
        let grant = (budget - *spent).min(want);
        *spent += grant;
        Ok(grant)
    }
}

fn quota(rps: u32, burst: u32, lease_ms: u64) -> Quota {
    Quota { rps, burst, lease_ms }
}

fn is_allow(d: &TrafficDecision) -> bool {
    matches!(d, TrafficDecision::Allow)
}

#[tokio::test]
async fn admits_up_to_global_budget_then_throttles() {
    let book = LeaseBook::new();
    let claims = FakeClaims::default();
    let q = quota(10, 5, 1_000); // budget 10 per window, chunk 5

    let mut allows = 0;
    for _ in 0..15 {
        if is_allow(&book.check("k", q, &claims, 0).await.unwrap()) {
            allows += 1;
        }
    }
    assert_eq!(allows, 10, "the window's global budget is 10");
}

#[tokio::test]
async fn amortizes_claims_over_burst_sized_chunks() {
    let book = LeaseBook::new();
    let claims = FakeClaims::default();
    let q = quota(10, 5, 1_000); // chunk 5 → 2 claims cover 10 requests

    for _ in 0..10 {
        book.check("k", q, &claims, 0).await.unwrap();
    }
    assert_eq!(claims.calls.load(Ordering::SeqCst), 2, "10 admits, chunk 5 → 2 backend claims");
}

#[tokio::test]
async fn exhausted_window_does_not_re_claim() {
    let book = LeaseBook::new();
    let claims = FakeClaims::default();
    let q = quota(1, 1, 1_000); // budget 1

    assert!(is_allow(&book.check("k", q, &claims, 0).await.unwrap()));
    // Many over-budget requests in the same window must not each hit the backend.
    for _ in 0..50 {
        assert!(!is_allow(&book.check("k", q, &claims, 5).await.unwrap()));
    }
    // 1 claim to grant the single token, 1 claim that discovered exhaustion. No more.
    assert_eq!(claims.calls.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn window_rollover_resets_budget() {
    let book = LeaseBook::new();
    let claims = FakeClaims::default();
    let q = quota(1, 1, 1_000);

    assert!(is_allow(&book.check("k", q, &claims, 0).await.unwrap()));
    assert!(!is_allow(&book.check("k", q, &claims, 10).await.unwrap())); // same window, spent
    assert!(is_allow(&book.check("k", q, &claims, 1_000).await.unwrap())); // next window
}

#[tokio::test]
async fn multiple_replicas_share_one_global_budget() {
    let claims = FakeClaims::default();
    let replica_a = LeaseBook::new();
    let replica_b = LeaseBook::new();
    let q = quota(10, 1, 1_000); // chunk 1 → exact global accounting across replicas

    let mut allows = 0;
    for _ in 0..20 {
        if is_allow(&replica_a.check("k", q, &claims, 0).await.unwrap()) {
            allows += 1;
        }
        if is_allow(&replica_b.check("k", q, &claims, 0).await.unwrap()) {
            allows += 1;
        }
    }
    assert_eq!(allows, 10, "two replicas together admit only the global budget");
}

#[tokio::test]
async fn distinct_keys_have_independent_budgets() {
    let book = LeaseBook::new();
    let claims = FakeClaims::default();
    let q = quota(1, 1, 1_000);

    assert!(is_allow(&book.check("a", q, &claims, 0).await.unwrap()));
    assert!(is_allow(&book.check("b", q, &claims, 0).await.unwrap()));
    assert!(!is_allow(&book.check("a", q, &claims, 0).await.unwrap()));
}

#[tokio::test]
async fn backend_failure_propagates() {
    let book = LeaseBook::new();
    let claims = FakeClaims::default();
    claims.fail.store(true, Ordering::SeqCst);

    let result = book.check("k", quota(10, 5, 1_000), &claims, 0).await;
    assert!(result.is_err(), "claim failure surfaces so the layer can apply fail policy");
}
