use std::collections::{BTreeSet, HashMap};

use chrono::{DateTime, Utc};

use crate::domain::observation::Observation;
use crate::domain::value_object::{Aggregation, EntityRef, MemberId, Metric, WindowId, WindowSize};
use crate::error::CounterError;

/// The aggregation + idempotency key for one folded window: `(entity, metric,
/// window)`. The durable flush is keyed by exactly this tuple, which is what makes
/// re-flushing a redelivered window a no-op UPSERT rather than a double-add.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WindowKey {
    pub entity: EntityRef,
    pub metric: Metric,
    pub window: WindowId,
}

/// Mutable fold state for one open window. A `Sum` metric accumulates a signed
/// running total; a `Cardinality` metric collects distinct members (deduped) to
/// be `PFADD`-ed downstream.
#[derive(Debug, Default)]
struct WindowAccumulator {
    sum: i64,
    members: BTreeSet<MemberId>,
}

/// The single delta emitted when a window closes — the result of collapsing N
/// observations into 1. For a `Sum` metric, `sum` is the net signed magnitude and
/// `unique_members` is empty; for a `Cardinality` metric, `unique_members` are the
/// distinct actors to fold into the HyperLogLog and `sum` is `0`.
#[derive(Debug, Clone, PartialEq)]
pub struct WindowDelta {
    pub key: WindowKey,
    pub sum: i64,
    pub unique_members: Vec<MemberId>,
}

impl WindowDelta {
    pub fn entity(&self) -> &EntityRef {
        &self.key.entity
    }

    pub fn metric(&self) -> Metric {
        self.key.metric
    }

    pub fn window(&self) -> WindowId {
        self.key.window
    }

    /// The single scalar contribution this window makes to a running total: the
    /// net `sum` for an additive metric, or the count of distinct members for a
    /// cardinality metric. Used by the durable and time-series tiers, which store
    /// scalars; the hot tier instead `PFADD`s the members directly to keep the
    /// cardinality estimate exact across windows.
    pub fn scalar(&self) -> i64 {
        match self.metric().aggregation() {
            Aggregation::Sum => self.sum,
            Aggregation::Cardinality => self.unique_members.len() as i64,
        }
    }
}

/// The windowed pre-aggregator — the heart of the write-amplification funnel.
///
/// It folds a stream of [`Observation`]s into one [`WindowDelta`] per `(entity,
/// metric, window)`, collapsing millions of `+1`s into a single delta before any
/// of them reaches a store. It is pure and clock-injected: an observation lands in
/// a window purely by its **event time**, and windows close purely against an
/// injected **watermark** — never `Utc::now()`. This makes the whole collapse
/// deterministic and unit-testable without a broker, a timer, or a store.
#[derive(Debug)]
pub struct WindowAggregator {
    size: WindowSize,
    buckets: HashMap<WindowKey, WindowAccumulator>,
}

impl WindowAggregator {
    pub fn new(size: WindowSize) -> Self {
        Self {
            size,
            buckets: HashMap::new(),
        }
    }

    /// Number of open (not-yet-flushed) windows currently held.
    pub fn pending(&self) -> usize {
        self.buckets.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buckets.is_empty()
    }

    /// Fold one observation into its window. A cardinality observation missing its
    /// member is a malformed event (the decode layer must always supply one),
    /// surfaced as `CTR-9001 DomainViolation`.
    pub fn fold(&mut self, obs: Observation) -> Result<(), CounterError> {
        let window = WindowId::for_event(self.size, obs.occurred_at);
        let key = WindowKey {
            entity: obs.entity,
            metric: obs.metric,
            window,
        };
        let acc = self.buckets.entry(key).or_default();

        match obs.metric.aggregation() {
            Aggregation::Sum => {
                // Saturating: a window total can never realistically overflow i64,
                // but we never want a panic on the ingestion path.
                acc.sum = acc.sum.saturating_add(obs.amount);
            }
            Aggregation::Cardinality => {
                let member = obs.unique_member.ok_or_else(|| CounterError::DomainViolation {
                    field: "unique_member".to_owned(),
                    message: format!(
                        "cardinality metric '{}' requires a member id",
                        obs.metric.as_str()
                    ),
                })?;
                acc.members.insert(member);
            }
        }
        Ok(())
    }

    /// Remove and return the deltas for every window that has fully elapsed
    /// relative to `watermark`. Open windows are retained for further folding. The
    /// result is deterministically ordered for stable testing and reproducible
    /// flush batches.
    pub fn drain_closed(&mut self, watermark: DateTime<Utc>) -> Vec<WindowDelta> {
        let closed: Vec<WindowKey> = self
            .buckets
            .keys()
            .filter(|k| k.window.is_closed_at(self.size, watermark))
            .cloned()
            .collect();
        self.remove_and_collect(closed)
    }

    /// Remove and return deltas for *all* open windows, regardless of watermark —
    /// for graceful shutdown, where holding back un-elapsed windows would lose
    /// data.
    pub fn drain_all(&mut self) -> Vec<WindowDelta> {
        let all: Vec<WindowKey> = self.buckets.keys().cloned().collect();
        self.remove_and_collect(all)
    }

    fn remove_and_collect(&mut self, keys: Vec<WindowKey>) -> Vec<WindowDelta> {
        let mut deltas: Vec<WindowDelta> = keys
            .into_iter()
            .filter_map(|key| {
                self.buckets.remove(&key).map(|acc| WindowDelta {
                    key,
                    sum: acc.sum,
                    unique_members: acc.members.into_iter().collect(),
                })
            })
            .collect();
        deltas.sort_by(|a, b| {
            (
                a.key.entity.kind.as_str(),
                a.key.entity.id.as_str(),
                a.key.metric.as_str(),
                a.key.window.index(),
            )
                .cmp(&(
                    b.key.entity.kind.as_str(),
                    b.key.entity.id.as_str(),
                    b.key.metric.as_str(),
                    b.key.window.index(),
                ))
        });
        deltas
    }
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;
    use error::AppError;

    use super::*;
    use crate::domain::value_object::{EntityId, EntityKind};

    fn size() -> WindowSize {
        WindowSize::from_millis(5_000).unwrap()
    }

    fn post(id: &str) -> EntityRef {
        EntityRef::new(EntityKind::Post, EntityId::new(id).unwrap())
    }

    fn at(millis: i64) -> DateTime<Utc> {
        Utc.timestamp_millis_opt(millis).unwrap()
    }

    /// The defining property: N observations collapse to 1 delta.
    #[test]
    fn collapses_n_views_into_one_delta() {
        let mut agg = WindowAggregator::new(size());
        for _ in 0..1_000_000 {
            let obs = Observation::sum(post("viral"), Metric::View, 1, at(1_000)).unwrap();
            agg.fold(obs).unwrap();
        }
        assert_eq!(agg.pending(), 1);

        let deltas = agg.drain_closed(at(10_000));
        assert_eq!(deltas.len(), 1);
        assert_eq!(deltas[0].sum, 1_000_000);
        assert_eq!(deltas[0].metric(), Metric::View);
        assert!(deltas[0].unique_members.is_empty());
        assert!(agg.is_empty());
    }

    #[test]
    fn signed_deltas_net_out() {
        let mut agg = WindowAggregator::new(size());
        agg.fold(Observation::sum(post("p"), Metric::Like, 1, at(1_000)).unwrap())
            .unwrap();
        agg.fold(Observation::sum(post("p"), Metric::Like, 1, at(1_001)).unwrap())
            .unwrap();
        agg.fold(Observation::sum(post("p"), Metric::Like, -1, at(1_002)).unwrap())
            .unwrap();

        let deltas = agg.drain_all();
        assert_eq!(deltas.len(), 1);
        assert_eq!(deltas[0].sum, 1); // +1 +1 -1
    }

    #[test]
    fn unique_members_are_deduped() {
        let mut agg = WindowAggregator::new(size());
        for who in ["alice", "alice", "bob", "carol", "bob"] {
            let obs = Observation::unique(
                post("p"),
                Metric::UniqueViewer,
                MemberId::new(who).unwrap(),
                at(1_000),
            )
            .unwrap();
            agg.fold(obs).unwrap();
        }
        let deltas = agg.drain_all();
        assert_eq!(deltas.len(), 1);
        assert_eq!(deltas[0].unique_members.len(), 3); // alice, bob, carol
        assert_eq!(deltas[0].sum, 0);
    }

    #[test]
    fn distinct_windows_yield_distinct_deltas() {
        let mut agg = WindowAggregator::new(size());
        // window 0 ([0,5000)) and window 2 ([10000,15000))
        agg.fold(Observation::sum(post("p"), Metric::View, 1, at(1_000)).unwrap())
            .unwrap();
        agg.fold(Observation::sum(post("p"), Metric::View, 1, at(12_000)).unwrap())
            .unwrap();
        assert_eq!(agg.pending(), 2);

        let deltas = agg.drain_all();
        assert_eq!(deltas.len(), 2);
        assert_eq!(deltas[0].window().index(), 0);
        assert_eq!(deltas[1].window().index(), 2);
    }

    #[test]
    fn distinct_metrics_on_same_entity_are_separate() {
        let mut agg = WindowAggregator::new(size());
        agg.fold(Observation::sum(post("p"), Metric::View, 5, at(1_000)).unwrap())
            .unwrap();
        agg.fold(Observation::sum(post("p"), Metric::Like, 2, at(1_000)).unwrap())
            .unwrap();
        let deltas = agg.drain_all();
        assert_eq!(deltas.len(), 2);
        // deterministic order: "like" < "view"
        assert_eq!(deltas[0].metric(), Metric::Like);
        assert_eq!(deltas[1].metric(), Metric::View);
    }

    #[test]
    fn watermark_holds_back_open_windows() {
        let mut agg = WindowAggregator::new(size());
        agg.fold(Observation::sum(post("p"), Metric::View, 1, at(12_000)).unwrap())
            .unwrap(); // window 2 = [10000,15000)

        // watermark inside the window → nothing closed yet
        let drained = agg.drain_closed(at(14_999));
        assert!(drained.is_empty());
        assert_eq!(agg.pending(), 1);

        // watermark at the window end → it closes
        let drained = agg.drain_closed(at(15_000));
        assert_eq!(drained.len(), 1);
        assert!(agg.is_empty());
    }

    #[test]
    fn cardinality_observation_without_member_is_rejected() {
        let mut agg = WindowAggregator::new(size());
        // Construct a malformed observation directly (bypassing the smart ctor).
        let bad = Observation {
            entity: post("p"),
            metric: Metric::UniqueViewer,
            amount: 0,
            unique_member: None,
            occurred_at: at(1_000),
        };
        let err = agg.fold(bad).unwrap_err();
        assert_eq!(err.error_code(), "CTR-9001");
    }
}
