use async_trait::async_trait;
use tracing::info;

use crate::application::port::EventPublisher;
use crate::domain::event::DomainEvent;
use crate::error::ModerationError;

/// A no-Kafka [`EventPublisher`] that traces events instead of emitting them.
/// Used for local development and tests where a broker is not wired.
#[derive(Default)]
pub struct LogEventPublisher;

#[async_trait]
impl EventPublisher for LogEventPublisher {
    async fn publish(&self, event: &DomainEvent) -> Result<(), ModerationError> {
        info!(
            event_type = event.event_type(),
            actor_id = %event.actor_id(),
            "moderation domain event (log publisher)"
        );
        Ok(())
    }
}
