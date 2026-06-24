//! The atomic-claim seam between the lease algorithm and the backing store.

use async_trait::async_trait;
use traffic::QuotaError;

/// Atomically leases tokens from a fleet-global budget for a `(key, window)` pair.
///
/// Split out from [`LeaseBook`](crate::LeaseBook) so the lease algorithm is unit-testable
/// against an in-memory fake, while the production implementation
/// ([`RedisClaimSource`](crate::RedisClaimSource)) is a single atomic Lua call.
/// Implementations **must be atomic across replicas** — concurrent claims may not overspend
/// the budget.
#[async_trait]
pub trait ClaimSource: Send + Sync {
    /// Grant up to `want` tokens from `(key, window)`'s remaining budget (window cap
    /// `budget`), refreshing the window state's TTL to `ttl_ms`. Returns the granted count
    /// in `0..=want` — `0` means the window's global budget is spent.
    async fn claim(
        &self,
        key: &str,
        window: u64,
        budget: u64,
        want: u64,
        ttl_ms: u64,
    ) -> Result<u64, QuotaError>;
}
