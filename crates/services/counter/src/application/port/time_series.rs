use async_trait::async_trait;

use crate::domain::{TimeSeriesBucket, TimeSeriesQuery, WindowDelta};
use crate::error::CounterError;

/// The cold time-series tier (ScyllaDB TWCS) — the historical per-bucket rollups,
/// TTL'd, never on the hot read path.
///
/// Writes append a window's scalar contribution into the appropriate time bucket
/// (the adapter rolls fine windows up into hour/day buckets). The one read,
/// [`range`](TimeSeriesStore::range), backs `GetTimeSeries`; it is explicitly NOT
/// sub-millisecond and is off the feed-render path.
#[async_trait]
pub trait TimeSeriesStore: Send + Sync + 'static {
    /// Append a closed window's scalar contribution to the historical series for
    /// its `(entity, metric)`. Idempotency follows the same `(entity, metric,
    /// window_id)` key as the ledger.
    async fn append(&self, delta: &WindowDelta) -> Result<(), CounterError>;

    /// Read the historical buckets for one `(entity, metric)` over a range, at the
    /// requested granularity. Ordered by `bucket_start` ascending.
    async fn range(&self, query: &TimeSeriesQuery) -> Result<Vec<TimeSeriesBucket>, CounterError>;
}
