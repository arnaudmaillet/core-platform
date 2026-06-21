use async_trait::async_trait;

use crate::domain::event::DomainEvent;
use crate::error::SocialGraphError;

/// Outbound Kafka event publisher port.
///
/// Implemented by [`crate::infrastructure::publisher::kafka_event_publisher::KafkaEventPublisher`].
///
/// # Topic mapping
///
/// | Event             | Kafka topic                  | Key               |
/// |-------------------|------------------------------|-------------------|
/// | ProfileFollowed   | `social-graph.followed`      | `{actor}:{target}`|
/// | ProfileUnfollowed | `social-graph.unfollowed`    | `{actor}:{target}`|
/// | ProfileBlocked    | `social-graph.blocked`       | `{actor}:{target}`|
///
/// `ProfileUnblocked` is intentionally not published downstream. Unblocking is
/// a user-local operation with no fan-out consequence for timeline engines.
#[async_trait]
pub trait EventPublisher: Send + Sync + 'static {
    async fn publish(&self, event: &DomainEvent) -> Result<(), SocialGraphError>;
}
