use async_trait::async_trait;

use crate::application::port::EventPublisher;
use crate::domain::event::DomainEvent;
use crate::error::MediaError;

/// Fallback [`EventPublisher`] that logs instead of producing to Kafka. Used when
/// no broker is configured (local dev) so the service still boots and runs.
pub struct LogEventPublisher;

#[async_trait]
impl EventPublisher for LogEventPublisher {
    async fn publish(&self, event: &DomainEvent) -> Result<(), MediaError> {
        tracing::info!(
            event_type = event.event_type(),
            asset_id = %event.asset_id(),
            "media event (log publisher; no Kafka configured)"
        );
        Ok(())
    }
}
