//! The distributed-coordination seam.
//!
//! `local` mode never touches this — it's served entirely by the in-process governor. A
//! `distributed` profile consults a [`QuotaBackend`] to enforce a *fleet-global* budget.
//! The backend lives in a separate crate (`traffic-redis`) so this pure crate — and the
//! transport layer — link no Redis; they know only this trait.

use std::fmt;

use crate::config::TrafficDecision;

/// Default lease window when a distributed profile omits `lease_ms` (validation normally
/// requires it, so this is a defensive fallback).
pub const DEFAULT_LEASE_MS: u64 = 1_000;

/// The fleet-global rate a distributed backend must enforce for a key, plus the lease
/// window and the burst that doubles as the per-replica lease chunk size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Quota {
    /// Global sustained rate, requests per second, shared across all replicas.
    pub rps: u32,
    /// Largest chunk a replica leases at once (fewer, larger chunks = less backend I/O,
    /// coarser cross-replica fairness).
    pub burst: u32,
    /// Lease window in milliseconds — the granularity at which the global budget refreshes.
    pub lease_ms: u64,
}

/// The distributed backend is unavailable (timeout, unreachable, script error). The caller
/// applies the profile's `on_backend_error` policy — it is never an admission decision itself.
#[derive(Debug, Clone)]
pub struct QuotaError(pub String);

impl fmt::Display for QuotaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "quota backend unavailable: {}", self.0)
    }
}

impl std::error::Error for QuotaError {}

/// Fleet-global rate-limit coordination, consulted only for `distributed` profiles.
///
/// Implementations **must amortize** backend I/O (e.g. lease a chunk of tokens and serve
/// locally) so the per-request hot path rarely crosses the network. `Ok` is an authoritative
/// global decision; `Err` means the backend is unreachable and the caller should fall back
/// per the profile's `on_backend_error` policy.
#[async_trait::async_trait]
pub trait QuotaBackend: Send + Sync + 'static {
    /// Charge one request under `key` against the global `quota`.
    async fn check(&self, key: &str, quota: Quota) -> Result<TrafficDecision, QuotaError>;
}
