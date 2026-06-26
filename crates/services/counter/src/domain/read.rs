//! The domain read model — the shapes served back on the query path, and the
//! pure derivation of the coarse popularity signal from them.

use serde::{Deserialize, Serialize};

use crate::domain::value_object::{
    EntityRef, Metric, MetricKind, PopularityScore, PopularityWeights,
};

/// A unique-cardinality estimate (what `PFCOUNT` returns for a HyperLogLog).
/// Distinct from a plain count to keep the approximate nature visible in the
/// type: this is never an exact distinct count, only an estimate within the
/// structure's error bound.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Cardinality(u64);

impl Cardinality {
    pub fn new(estimate: u64) -> Self {
        Self(estimate)
    }

    pub fn value(&self) -> u64 {
        self.0
    }
}

/// One served magnitude for an entity: the metric, its value, and the provenance
/// (`Exact` vs `Approximate`) so the caller can render precision honestly. Carries
/// no actor identity — the boundary that keeps "how many?" separate from "who?".
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CounterValue {
    pub metric: Metric,
    pub value: i64,
    pub kind: MetricKind,
}

impl CounterValue {
    /// Build a value, stamping the provenance from the metric's intrinsic kind.
    pub fn new(metric: Metric, value: i64) -> Self {
        Self {
            metric,
            value,
            kind: metric.kind(),
        }
    }
}

/// The set of served magnitudes for one entity reference.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CountSnapshot {
    pub entity: EntityRef,
    pub values: Vec<CounterValue>,
}

impl CountSnapshot {
    pub fn new(entity: EntityRef, values: Vec<CounterValue>) -> Self {
        Self { entity, values }
    }

    /// The value for a metric, if present in this snapshot.
    pub fn get(&self, metric: Metric) -> Option<i64> {
        self.values
            .iter()
            .find(|v| v.metric == metric)
            .map(|v| v.value)
    }

    /// Derive the coarse popularity signal for this entity from its magnitudes.
    ///
    /// A deterministic weighted blend — this is the slow-loop transform whose
    /// output is published on `counter.v1.popularity` and projected by `search`.
    /// It is intentionally cheap and side-effect-free: popularity is recomputed
    /// from already-aggregated counts, never from per-event work.
    pub fn popularity(&self, weights: &PopularityWeights) -> PopularityScore {
        let total: f64 = self
            .values
            .iter()
            .map(|v| weights.for_metric(v.metric) * v.value as f64)
            .sum();
        PopularityScore::new(total)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::value_object::{EntityId, EntityKind};

    fn post() -> EntityRef {
        EntityRef::new(EntityKind::Post, EntityId::new("p1").unwrap())
    }

    #[test]
    fn counter_value_stamps_provenance_from_metric() {
        assert_eq!(CounterValue::new(Metric::Like, 5).kind, MetricKind::Exact);
        assert_eq!(
            CounterValue::new(Metric::View, 9).kind,
            MetricKind::Approximate
        );
    }

    #[test]
    fn popularity_is_weighted_blend() {
        let snap = CountSnapshot::new(
            post(),
            vec![
                CounterValue::new(Metric::View, 100), // 100 * 0.1 = 10
                CounterValue::new(Metric::Like, 10),  // 10  * 1.0 = 10
                CounterValue::new(Metric::Share, 5),  // 5   * 3.0 = 15
                CounterValue::new(Metric::Follower, 999), // weight 0 → ignored
            ],
        );
        let score = snap.popularity(&PopularityWeights::default());
        assert_eq!(score.value(), 35.0);
    }

    #[test]
    fn empty_snapshot_has_zero_popularity() {
        let snap = CountSnapshot::new(post(), vec![]);
        assert_eq!(
            snap.popularity(&PopularityWeights::default()),
            PopularityScore::ZERO
        );
    }

    #[test]
    fn get_returns_metric_value() {
        let snap = CountSnapshot::new(post(), vec![CounterValue::new(Metric::View, 42)]);
        assert_eq!(snap.get(Metric::View), Some(42));
        assert_eq!(snap.get(Metric::Like), None);
    }
}
