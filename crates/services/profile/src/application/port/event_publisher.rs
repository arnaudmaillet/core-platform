use async_trait::async_trait;

use crate::domain::event::DomainEvent;
use crate::error::ProfileError;

/// Outbound port for publishing profile domain events.
///
/// Command handlers persist the aggregate to the system of record first, then
/// drain its pending events and publish them through this port (durable-first
/// ordering — the event is a denormalization notification, never the source of
/// truth). The Kafka adapter emits to `profile.v1.events`; a no-op adapter backs
/// broker-free composition (tests, no-Kafka deployments).
#[async_trait]
pub trait EventPublisher: Send + Sync + 'static {
    async fn publish(&self, event: &DomainEvent) -> Result<(), ProfileError>;
}
