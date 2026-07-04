//! The slow-loop popularity publisher: read an entity's coarse magnitudes, derive
//! the popularity score, and emit it on `counter.v1.popularity`.
//!
//! This is what unblocks `search`'s `PopularityScore` ranking input. It is
//! deliberately decoupled from ingestion — driven on a slow cadence by the worker
//! (Phase 5), never per-event — so updating a ranking signal never amplifies the
//! firehose.

use std::sync::Arc;

use crate::application::port::{CounterStore, SignalPublisher};
use crate::domain::{EntityRef, Metric, PopularityWeights};
use crate::error::CounterError;

/// The metrics that feed the coarse popularity score. Reading only these keeps the
/// slow-loop read cheap.
const POPULARITY_METRICS: [Metric; 4] =
    [Metric::View, Metric::Like, Metric::Share, Metric::Comment];

/// Derives and publishes the coarse popularity signal for an entity.
pub struct PopularityPublisher {
    hot: Arc<dyn CounterStore>,
    publisher: Arc<dyn SignalPublisher>,
    weights: PopularityWeights,
}

impl PopularityPublisher {
    pub fn new(hot: Arc<dyn CounterStore>, publisher: Arc<dyn SignalPublisher>) -> Self {
        Self {
            hot,
            publisher,
            weights: PopularityWeights::default(),
        }
    }

    pub fn with_weights(mut self, weights: PopularityWeights) -> Self {
        self.weights = weights;
        self
    }

    /// Read the entity's popularity-relevant magnitudes, blend them, and publish.
    pub async fn publish(&self, entity: &EntityRef) -> Result<(), CounterError> {
        let snapshots = self
            .hot
            .read(std::slice::from_ref(entity), &POPULARITY_METRICS)
            .await?;
        let score = snapshots
            .first()
            .map(|snap| snap.popularity(&self.weights))
            .unwrap_or_default();
        self.publisher.publish_popularity(entity, score).await
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
    async fn derives_and_publishes_weighted_score() {
        let fx = Fixture::new();
        // 100 views (*0.1=10) + 5 shares (*3.0=15) = 25
        fx.hot.seed(&post("p1"), Metric::View, 100);
        fx.hot.seed(&post("p1"), Metric::Share, 5);

        let pp = fx.popularity_publisher();
        pp.publish(&post("p1")).await.unwrap();

        assert_eq!(fx.publisher.last_score(&post("p1")), Some(25.0));
    }

    #[tokio::test]
    async fn unknown_entity_publishes_zero() {
        let fx = Fixture::new();
        let pp = fx.popularity_publisher();
        pp.publish(&post("ghost")).await.unwrap();
        assert_eq!(fx.publisher.last_score(&post("ghost")), Some(0.0));
    }
}
