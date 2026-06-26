use chrono::{DateTime, Utc};

/// The injected wall clock. Ledger record-time (`recorded_at`), checkpoint times
/// and hold/retention evaluations all read it through this seam, so handlers are
/// deterministic under a fixed clock in tests. The real adapter (Phase 4) reads
/// `Utc::now`; the fake returns a pinned instant.
///
/// Not `async` and not fallible — reading the clock cannot fail or block.
pub trait Clock: Send + Sync + 'static {
    fn now(&self) -> DateTime<Utc>;
}
