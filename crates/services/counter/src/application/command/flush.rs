//! The write-side use case: flush closed window deltas across the three storage
//! tiers. This is the application service the worker drives after draining the
//! domain [`WindowAggregator`](crate::domain::WindowAggregator); the windowing
//! itself is stateful and lives in the worker (Phase 5), not here.
//!
//! Like `search`/`moderation`, this is a plain application-service struct (not a
//! `cqrs::CommandHandler`): it returns a rich [`FlushReport`] for metrics rather
//! than a bare `Result<()>`.

use std::sync::Arc;

use crate::application::port::{CounterLedger, CounterStore, FlushOutcome, TimeSeriesStore};
use crate::domain::WindowDelta;
use crate::error::CounterError;

/// What a flush batch did, for observability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FlushReport {
    /// Windows applied for the first time (durable total advanced).
    pub applied: usize,
    /// Windows that were redeliveries — already durably flushed, so skipped across
    /// all tiers (no double-add).
    pub already_applied: usize,
}

impl FlushReport {
    pub fn total(&self) -> usize {
        self.applied + self.already_applied
    }
}

/// Fans a closed window delta out to the hot tier, the durable ledger, and the
/// cold time-series.
///
/// **The durable ledger's idempotency gates every side effect.** `flush_window`
/// is keyed by `(entity, metric, window_id)`; only when it reports
/// [`FlushOutcome::Applied`] (a first-time flush) does the delta also land in the
/// hot tier and the time-series. A Kafka redelivery therefore re-runs the whole
/// fan-out as a true no-op — no double-counted view, no double-incremented HLL.
///
/// Ingestion **fails closed**: any tier error propagates so the consumer retries
/// then dead-letters, rather than silently dropping a window.
pub struct DeltaFlusher {
    hot: Arc<dyn CounterStore>,
    ledger: Arc<dyn CounterLedger>,
    series: Arc<dyn TimeSeriesStore>,
}

impl DeltaFlusher {
    pub fn new(
        hot: Arc<dyn CounterStore>,
        ledger: Arc<dyn CounterLedger>,
        series: Arc<dyn TimeSeriesStore>,
    ) -> Self {
        Self {
            hot,
            ledger,
            series,
        }
    }

    /// Flush a batch of closed window deltas. Returns once every delta has reached
    /// a terminal tier outcome, or on the first propagating error.
    pub async fn flush(&self, deltas: &[WindowDelta]) -> Result<FlushReport, CounterError> {
        let mut report = FlushReport::default();
        for delta in deltas {
            match self.ledger.flush_window(delta).await? {
                FlushOutcome::Applied => {
                    self.hot.apply_delta(delta).await?;
                    self.series.append(delta).await?;
                    report.applied += 1;
                }
                FlushOutcome::AlreadyApplied => {
                    report.already_applied += 1;
                }
            }
        }
        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};

    use super::*;
    use crate::application::fakes::Fixture;
    use crate::domain::{
        EntityId, EntityKind, EntityRef, Metric, Observation, WindowAggregator, WindowSize,
    };

    fn post(id: &str) -> EntityRef {
        EntityRef::new(EntityKind::Post, EntityId::new(id).unwrap())
    }

    fn one_view_window(id: &str, views: i64) -> Vec<WindowDelta> {
        let mut agg = WindowAggregator::new(WindowSize::from_millis(5_000).unwrap());
        let at = Utc.timestamp_millis_opt(1_000).unwrap();
        for _ in 0..views {
            agg.fold(Observation::sum(post(id), Metric::View, 1, at).unwrap())
                .unwrap();
        }
        agg.drain_all()
    }

    #[tokio::test]
    async fn first_flush_lands_in_all_tiers() {
        let fx = Fixture::new();
        let flusher = fx.delta_flusher();
        let deltas = one_view_window("p1", 1000);

        let report = flusher.flush(&deltas).await.unwrap();
        assert_eq!(report.applied, 1);
        assert_eq!(report.already_applied, 0);

        assert_eq!(fx.ledger.total(&post("p1"), Metric::View), Some(1000));
        assert_eq!(fx.hot.value(&post("p1"), Metric::View), 1000);
        assert_eq!(fx.series.bucket_count(&post("p1"), Metric::View), 1);
    }

    #[tokio::test]
    async fn redelivered_window_is_a_no_op_across_all_tiers() {
        let fx = Fixture::new();
        let flusher = fx.delta_flusher();
        let deltas = one_view_window("p1", 1000);

        flusher.flush(&deltas).await.unwrap();
        // Exact same window (same window_id) redelivered.
        let report = flusher.flush(&deltas).await.unwrap();

        assert_eq!(report.applied, 0);
        assert_eq!(report.already_applied, 1);
        // Hot counter did NOT double — idempotency gated the side effects.
        assert_eq!(fx.hot.value(&post("p1"), Metric::View), 1000);
        assert_eq!(fx.ledger.total(&post("p1"), Metric::View), Some(1000));
        assert_eq!(fx.series.bucket_count(&post("p1"), Metric::View), 1);
    }
}
