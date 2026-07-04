//! Outbound event publishing for `profile.v1.events`.

pub mod kafka_event_publisher;
pub mod wire;

pub use kafka_event_publisher::KafkaProfileEventPublisher;

use async_trait::async_trait;

use crate::application::port::EventPublisher;
use crate::domain::event::DomainEvent;
use crate::error::ProfileError;

/// A no-op publisher for broker-free composition (tests, no-Kafka deployments).
/// Drained events are dropped — the durable write in the system of record is the
/// source of truth.
pub struct NoopEventPublisher;

#[async_trait]
impl EventPublisher for NoopEventPublisher {
    async fn publish(&self, _event: &DomainEvent) -> Result<(), ProfileError> {
        Ok(())
    }
}
