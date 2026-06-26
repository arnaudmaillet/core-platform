use async_trait::async_trait;

use crate::domain::{EntityRef, Metric, WindowDelta};
use crate::error::CounterError;

/// The result of an idempotent durable flush.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlushOutcome {
    /// The window was applied to the durable total for the first time.
    Applied,
    /// This `(entity, metric, window_id)` was already flushed — a redelivery. The
    /// durable total was NOT changed (no double-add). The consumer commits
    /// regardless.
    AlreadyApplied,
}

/// The warm counter tier (Postgres) — the auditable materialized totals and the
/// reconciliation ledger. Low write rate (batched window flushes only), the
/// source of the cache-aside fallback when the hot tier is cold.
///
/// The idempotency guarantee lives here: [`flush_window`](CounterLedger::flush_window)
/// is keyed by `(entity, metric, window_id)`, so a worker crash + Kafka
/// redelivery re-applies the *same* window as a no-op. This is what makes
/// at-least-once delivery safe for the durable total.
#[async_trait]
pub trait CounterLedger: Send + Sync + 'static {
    /// Apply a closed window's scalar contribution to the durable total,
    /// idempotently on `(entity, metric, window_id)`. Returns
    /// [`FlushOutcome::AlreadyApplied`] for a redelivery.
    async fn flush_window(&self, delta: &WindowDelta) -> Result<FlushOutcome, CounterError>;

    /// Read the durable total for one `(entity, metric)` — the cache-aside
    /// fallback used when the hot tier misses or is unavailable. `None` when the
    /// pair has never been flushed.
    async fn read_total(
        &self,
        entity: &EntityRef,
        metric: Metric,
    ) -> Result<Option<i64>, CounterError>;

    /// Reconciliation-only: overwrite the durable total to an authoritative value,
    /// healing accumulated drift against the owning source-of-record. Subsequent
    /// window flushes resume adding from the corrected baseline.
    async fn set_total(
        &self,
        entity: &EntityRef,
        metric: Metric,
        value: i64,
    ) -> Result<(), CounterError>;
}
