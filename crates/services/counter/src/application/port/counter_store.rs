use async_trait::async_trait;

use crate::domain::{CountSnapshot, EntityRef, Metric, TrendingItem, TrendingScope, WindowDelta};
use crate::error::CounterError;

/// The hot counter tier (Redis) — the only store on the sub-millisecond read
/// path, and the canonical home of the live magnitudes, the HyperLogLog unique
/// estimators, and the Count-Min-Sketch trending sketches.
///
/// Writes are *deltas*: a [`WindowDelta`] folds into the live counter (`INCRBY`
/// the net sum) or the HyperLogLog (`PFADD` the distinct members), and feeds the
/// trending sketch. Reads are a batched multi-get (`HMGET` / `PFCOUNT`). The
/// adapter (Phase 4) owns the shard re-aggregation and the fail-open behaviour;
/// this port is the contract the handlers hold.
#[async_trait]
pub trait CounterStore: Send + Sync + 'static {
    /// Fold one closed window's delta into the hot tier (live counter / HLL +
    /// trending). Idempotency is NOT this tier's job — at-least-once redelivery of
    /// approximate metrics is tolerated, and exact metrics are corrected by the
    /// reconciliation loop; durable idempotency lives in [`CounterLedger`].
    async fn apply_delta(&self, delta: &WindowDelta) -> Result<(), CounterError>;

    /// Read the requested metrics for a batch of entities in one round-trip — the
    /// `BatchGetCounters` hot path. Returns one snapshot per entity, in the same
    /// order as `entities`.
    async fn read(
        &self,
        entities: &[EntityRef],
        metrics: &[Metric],
    ) -> Result<Vec<CountSnapshot>, CounterError>;

    /// Top-`limit` trending entities for a scope, ranked from the Count-Min Sketch
    /// + bounded heap. Approximate by design.
    async fn top_k(
        &self,
        scope: TrendingScope,
        scope_key: Option<&str>,
        metric: Metric,
        limit: usize,
    ) -> Result<Vec<TrendingItem>, CounterError>;

    /// Reconciliation-only: overwrite the hot counter for an exact (sum) metric to
    /// an authoritative value, healing accumulated drift. Not an additive delta —
    /// the live counter is set, not incremented.
    async fn overwrite(
        &self,
        entity: &EntityRef,
        metric: Metric,
        value: i64,
    ) -> Result<(), CounterError>;
}
