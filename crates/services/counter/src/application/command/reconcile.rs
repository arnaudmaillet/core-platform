//! The reconciliation use case — the self-heal that makes an *exact* metric
//! trustworthy despite at-least-once delivery.
//!
//! The hot counter for a like/follow drifts: a redelivered window that the ledger
//! gated still applied to the HLL-free sum on a partial failure, a lost window
//! under-counts, etc. Periodically the reconciler asks the owning service for the
//! true magnitude and, if the durable total has drifted beyond tolerance, heals
//! both the durable total and the hot counter to the authoritative value. Approximate
//! metrics (views) are never reconciled — they are accepted within their error bound.

use std::sync::Arc;

use crate::application::port::{CounterLedger, CounterStore, ReconciliationSource};
use crate::domain::{EntityRef, Metric, MetricKind};
use crate::error::CounterError;

/// What a single reconciliation did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReconcileOutcome {
    /// The durable total already matched the authoritative count.
    InSync,
    /// Drift existed but stayed within tolerance — left untouched to avoid churn.
    WithinTolerance { drift: i64 },
    /// Drift exceeded tolerance; the total + hot counter were healed.
    Corrected { from: i64, to: i64 },
    /// Not reconcilable: an approximate metric, or the source does not own this
    /// `(entity, metric)`.
    NotApplicable,
}

/// Reconciles exact counters against the owning source-of-record.
pub struct Reconciler {
    store: Arc<dyn CounterStore>,
    ledger: Arc<dyn CounterLedger>,
    source: Arc<dyn ReconciliationSource>,
    /// Absolute drift tolerated before a correction is applied.
    tolerance: i64,
}

impl Reconciler {
    pub fn new(
        store: Arc<dyn CounterStore>,
        ledger: Arc<dyn CounterLedger>,
        source: Arc<dyn ReconciliationSource>,
        tolerance: i64,
    ) -> Self {
        Self {
            store,
            ledger,
            source,
            tolerance,
        }
    }

    /// Reconcile one `(entity, metric)`. Only exact metrics are reconcilable.
    pub async fn reconcile(
        &self,
        entity: &EntityRef,
        metric: Metric,
    ) -> Result<ReconcileOutcome, CounterError> {
        if metric.kind() != MetricKind::Exact {
            return Ok(ReconcileOutcome::NotApplicable);
        }

        let authoritative = match self.source.authoritative_count(entity, metric).await? {
            Some(value) => value,
            None => return Ok(ReconcileOutcome::NotApplicable),
        };

        let current = self.ledger.read_total(entity, metric).await?.unwrap_or(0);
        let drift = authoritative - current;

        if drift == 0 {
            return Ok(ReconcileOutcome::InSync);
        }
        if drift.abs() <= self.tolerance {
            return Ok(ReconcileOutcome::WithinTolerance { drift });
        }

        // Heal both tiers to truth. The hot overwrite is a set, not a delta.
        self.ledger.set_total(entity, metric, authoritative).await?;
        self.store.overwrite(entity, metric, authoritative).await?;

        // CTR-5002 is the drift alarm; emitted as an operational signal (the
        // correction itself succeeded, so this is a warning, not a returned error).
        tracing::warn!(
            code = "CTR-5002",
            entity_kind = entity.kind.as_str(),
            entity_id = entity.id.as_str(),
            metric = metric.as_str(),
            from = current,
            to = authoritative,
            "counter drift exceeded tolerance; corrected against source-of-record"
        );

        Ok(ReconcileOutcome::Corrected {
            from: current,
            to: authoritative,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::fakes::Fixture;
    use crate::domain::{EntityId, EntityKind};

    fn post(id: &str) -> EntityRef {
        EntityRef::new(EntityKind::Post, EntityId::new(id).unwrap())
    }

    #[tokio::test]
    async fn corrects_drift_beyond_tolerance() {
        let fx = Fixture::new();
        // Durable total drifted low; SoR says 1000.
        fx.ledger.seed_total(&post("p1"), Metric::Like, 940);
        fx.source.set_authoritative(&post("p1"), Metric::Like, 1000);

        let reconciler = fx.reconciler(5);
        let outcome = reconciler.reconcile(&post("p1"), Metric::Like).await.unwrap();

        assert_eq!(outcome, ReconcileOutcome::Corrected { from: 940, to: 1000 });
        assert_eq!(fx.ledger.total(&post("p1"), Metric::Like), Some(1000));
        assert_eq!(fx.hot.value(&post("p1"), Metric::Like), 1000);
    }

    #[tokio::test]
    async fn leaves_small_drift_untouched() {
        let fx = Fixture::new();
        fx.ledger.seed_total(&post("p1"), Metric::Like, 998);
        fx.source.set_authoritative(&post("p1"), Metric::Like, 1000);

        let outcome = fx.reconciler(5).reconcile(&post("p1"), Metric::Like).await.unwrap();
        assert_eq!(outcome, ReconcileOutcome::WithinTolerance { drift: 2 });
        // Unchanged.
        assert_eq!(fx.ledger.total(&post("p1"), Metric::Like), Some(998));
    }

    #[tokio::test]
    async fn approximate_metrics_are_not_reconciled() {
        let fx = Fixture::new();
        fx.source.set_authoritative(&post("p1"), Metric::View, 1000);
        let outcome = fx.reconciler(5).reconcile(&post("p1"), Metric::View).await.unwrap();
        assert_eq!(outcome, ReconcileOutcome::NotApplicable);
    }

    #[tokio::test]
    async fn unknown_to_source_is_not_applicable() {
        let fx = Fixture::new();
        // No authoritative value seeded.
        let outcome = fx.reconciler(5).reconcile(&post("ghost"), Metric::Like).await.unwrap();
        assert_eq!(outcome, ReconcileOutcome::NotApplicable);
    }

    fn profile(id: &str) -> EntityRef {
        EntityRef::new(EntityKind::Profile, EntityId::new(id).unwrap())
    }

    #[tokio::test]
    async fn candidate_scan_lists_only_reconcilable_metrics_and_pages() {
        use crate::application::port::{CounterLedger, reconcile_cursor};

        let fx = Fixture::new();
        fx.ledger.seed_total(&profile("a"), Metric::Follower, 10);
        fx.ledger.seed_total(&profile("a"), Metric::Following, 5);
        fx.ledger.seed_total(&profile("b"), Metric::Follower, 20);
        fx.ledger.seed_total(&post("p"), Metric::View, 999); // approximate → excluded
        fx.ledger.seed_total(&post("p"), Metric::Like, 7); // no source RPC → excluded

        // Full page: exactly the three follower/following pairs, cursor-ordered.
        let all = fx.ledger.list_reconcilable(None, 100).await.unwrap();
        assert_eq!(all.len(), 3);
        assert!(all.iter().all(|(e, m)| e.kind == EntityKind::Profile
            && matches!(m, Metric::Follower | Metric::Following)));

        // Paging: first 2, then the rest after the cursor.
        let first = fx.ledger.list_reconcilable(None, 2).await.unwrap();
        assert_eq!(first.len(), 2);
        let (last_e, last_m) = first.last().unwrap();
        let cursor = reconcile_cursor(last_e, *last_m);
        let rest = fx.ledger.list_reconcilable(Some(&cursor), 100).await.unwrap();
        assert_eq!(rest.len(), 1);
    }
}
