use async_trait::async_trait;
use tracing::info;

use crate::application::port::EventPublisher;
use crate::domain::event::DomainEvent;
use crate::error::AuthError;

use super::event_key;

/// A no-Kafka [`EventPublisher`] that traces events instead of emitting them.
/// Used for local development and tests where a broker is not wired.
#[derive(Default)]
pub struct LogEventPublisher;

#[async_trait]
impl EventPublisher for LogEventPublisher {
    async fn publish(&self, event: &DomainEvent) -> Result<(), AuthError> {
        info!(
            event_type = event.event_type(),
            account_id = %event_key(event),
            "auth domain event (log publisher)"
        );
        Ok(())
    }
}
