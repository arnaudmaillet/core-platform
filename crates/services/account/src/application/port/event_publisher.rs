use async_trait::async_trait;

use crate::domain::event::DomainEvent;
use crate::error::AccountError;

/// Outbound port for publishing account domain events to the `account.v1.events`
/// Kafka topic.
///
/// The repository persists durably first, then drains the aggregate's events and
/// publishes them — the durable write is the source of truth, the event is the
/// notification (consumed by the `audit` compliance plane, among others). A publish
/// failure surfaces as [`AccountError::EventPublishFailed`].
#[async_trait]
pub trait EventPublisher: Send + Sync + 'static {
    async fn publish(&self, event: &DomainEvent) -> Result<(), AccountError>;
}
