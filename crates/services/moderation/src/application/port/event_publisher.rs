use async_trait::async_trait;

use crate::domain::event::DomainEvent;
use crate::error::ModerationError;

/// Outbound port for publishing domain events to the `moderation.v1.events` Kafka
/// topic (Phase 4 adapter), keyed by `actor_id` for per-actor ordering.
///
/// Handlers persist durably first, then publish — the durable write is the source
/// of truth, the event is the Plane B denormalization notification. A publish
/// failure surfaces so the caller can decide (the adapter may back an outbox).
#[async_trait]
pub trait EventPublisher: Send + Sync + 'static {
    async fn publish(&self, event: &DomainEvent) -> Result<(), ModerationError>;
}
