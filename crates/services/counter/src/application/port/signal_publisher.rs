use async_trait::async_trait;

use crate::domain::{EntityRef, PopularityScore};
use crate::error::CounterError;

/// Publishes the coarse popularity signal on `counter.v1.popularity`.
///
/// This is the only thing counter-analytics publishes. It is a *slow-loop*
/// emission derived from already-aggregated magnitudes (never per-event), consumed
/// by `search` (its `PopularityScore` ranking input) and `timeline`. The concrete
/// adapter (Phase 4) is a Kafka producer; this port keeps the slow-loop publisher
/// (Phase 5) free of any transport detail.
#[async_trait]
pub trait SignalPublisher: Send + Sync + 'static {
    /// Emit the current coarse popularity score for an entity.
    async fn publish_popularity(
        &self,
        entity: &EntityRef,
        score: PopularityScore,
    ) -> Result<(), CounterError>;
}
