use async_trait::async_trait;

use crate::domain::event::DomainEvent;
use crate::error::AuthError;

/// Outbound port for publishing domain events to the `auth.v1.events` Kafka topic
/// (Phase 4 adapter).
///
/// Handlers persist durably first, then publish — the durable write is the source
/// of truth, the event is the notification. A publish failure surfaces so the
/// caller can decide (the adapter may itself back an outbox).
#[async_trait]
pub trait EventPublisher: Send + Sync + 'static {
    async fn publish(&self, event: &DomainEvent) -> Result<(), AuthError>;
}
