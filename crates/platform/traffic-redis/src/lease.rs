//! Local lease cache + the windowed-budget algorithm (transport- and Redis-agnostic).

use std::sync::Arc;
use std::time::Duration;

use dashmap::DashMap;
use tokio::sync::Mutex;
use traffic::{Quota, QuotaError, TrafficDecision};

use crate::claim::ClaimSource;

/// One replica's view of a key's lease for the current window.
#[derive(Clone, Copy)]
struct Lease {
    /// The window this lease state belongs to (`now_ms / lease_ms`).
    window: u64,
    /// Tokens still available locally without contacting the backend.
    remaining: u64,
    /// The window's *global* budget is spent — short-circuit further requests this window
    /// without re-claiming, so an over-budget flood can't hammer the backend.
    exhausted: bool,
}

/// Per-key local lease cache. Each replica serves requests from a locally-held chunk of the
/// global budget, only crossing to the [`ClaimSource`] when its chunk runs out — so backend
/// I/O is amortized over `burst` requests per key per replica.
#[derive(Default)]
pub struct LeaseBook {
    leases: DashMap<String, Arc<Mutex<Lease>>>,
}

impl LeaseBook {
    pub fn new() -> Self {
        Self::default()
    }

    /// Charge one request under `key` against the global `quota`. `now_ms` (epoch millis) is
    /// injected so the algorithm is testable without a clock.
    ///
    /// Per-key requests serialize on a lightweight async mutex: the common path (local lease
    /// hit) holds it only for an integer decrement; the lock spans a claim only on refill,
    /// which also coalesces a burst of same-key refills into one backend call.
    pub async fn check<C: ClaimSource>(
        &self,
        key: &str,
        quota: Quota,
        claims: &C,
        now_ms: u64,
    ) -> Result<TrafficDecision, QuotaError> {
        let lease_ms = quota.lease_ms.max(1);
        let window = now_ms / lease_ms;
        let budget = window_budget(quota.rps, lease_ms);
        let want = chunk(quota.burst, budget);
        let ttl_ms = lease_ms.saturating_mul(2).max(lease_ms + 1_000);

        let cell = self
            .leases
            .entry(key.to_owned())
            .or_insert_with(|| Arc::new(Mutex::new(Lease { window, remaining: 0, exhausted: false })))
            .clone();
        let mut lease = cell.lock().await;

        // Window rolled forward — the global budget reset, so does our local view.
        if lease.window != window {
            lease.window = window;
            lease.remaining = 0;
            lease.exhausted = false;
        }

        if lease.remaining > 0 {
            lease.remaining -= 1;
            return Ok(TrafficDecision::Allow);
        }

        // Budget is monotonic within a window, so once spent it stays spent — serve the
        // throttle locally rather than re-claiming on every excess request.
        if lease.exhausted {
            return Ok(TrafficDecision::Throttle { retry_after: until_next_window(now_ms, lease_ms) });
        }

        match claims.claim(key, window, budget, want, ttl_ms).await? {
            0 => {
                lease.exhausted = true;
                Ok(TrafficDecision::Throttle { retry_after: until_next_window(now_ms, lease_ms) })
            }
            granted => {
                lease.remaining = granted - 1; // consume one for this request
                Ok(TrafficDecision::Allow)
            }
        }
    }

    /// Drop lease entries whose window has passed — best-effort and lock-free (entries
    /// currently in use are kept). Call periodically to bound memory under churny keyspaces.
    pub fn prune(&self, now_ms: u64, lease_ms: u64) {
        let current = now_ms / lease_ms.max(1);
        self.leases.retain(|_, cell| match cell.try_lock() {
            Ok(lease) => lease.window >= current,
            Err(_) => true,
        });
    }

    /// Number of keys with a live lease entry — for a cardinality gauge.
    pub fn tracked_keys(&self) -> usize {
        self.leases.len()
    }
}

/// Global token budget for one window: `ceil(rps * lease_ms / 1000)`, at least 1.
pub fn window_budget(rps: u32, lease_ms: u64) -> u64 {
    (rps as u64 * lease_ms).div_ceil(1_000).max(1)
}

/// How many tokens to lease at once: `burst`, capped at the window budget.
fn chunk(burst: u32, budget: u64) -> u64 {
    (burst.max(1) as u64).min(budget)
}

fn until_next_window(now_ms: u64, lease_ms: u64) -> Duration {
    Duration::from_millis(lease_ms - (now_ms % lease_ms))
}
