use async_trait::async_trait;

use crate::domain::event::{DomainEvent, MessageEvent};
use crate::error::ChatError;

/// Outbound port for emitting domain events to the messaging backbone (Kafka).
///
/// Two channels because the two event families have different fan-out shapes:
/// conversation-lifecycle events drive cache invalidation and Audience-Plane
/// attach/detach wiring, while message events are forked by the routing layer
/// into the Member-Plane broadcast and the stripped Audience-Plane shadow.
///
/// The publish is best-effort with respect to the real-time planes; durability
/// is provided by the ScyllaDB write that precedes it. Handlers therefore always
/// persist first, then publish.
#[async_trait]
pub trait EventPublisher: Send + Sync + 'static {
    /// Publishes a conversation-lifecycle event (created, published, unpublished,
    /// member joined/left).
    async fn publish_conversation(&self, event: &DomainEvent) -> Result<(), ChatError>;

    /// Publishes a message event (a durably-written message ready for fan-out).
    async fn publish_message(&self, event: &MessageEvent) -> Result<(), ChatError>;
}
