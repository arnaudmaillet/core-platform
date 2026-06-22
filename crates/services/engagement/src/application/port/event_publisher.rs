use async_trait::async_trait;

use crate::domain::event::reaction_event::ReactionKafkaEvent;
use crate::error::EngagementError;

/// Port for publishing engagement domain events to Kafka.
///
/// The topic is `engagement.reactions`. Messages are keyed by
/// `{post_id}:{profile_id}` to preserve per-profile ordering within each
/// Kafka partition — the write-behind consumer processes events for the same
/// pair sequentially.
#[async_trait]
pub trait EngagementEventPublisher: Send + Sync + 'static {
    async fn publish_reaction_event(&self, event: &ReactionKafkaEvent) -> Result<(), EngagementError>;
}
