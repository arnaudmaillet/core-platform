use async_trait::async_trait;

use crate::application::port::EventPublisher;
use crate::domain::event::{DomainEvent, MessageEvent};
use crate::error::ChatError;

/// Placeholder [`EventPublisher`] that logs events instead of producing to Kafka.
///
/// Lets the service run end-to-end before the messaging backbone is wired. Phase
/// 7 replaces this with a Kafka producer-backed implementation; the application
/// handlers are unaffected because they depend only on the [`EventPublisher`]
/// port.
pub struct LogEventPublisher;

#[async_trait]
impl EventPublisher for LogEventPublisher {
    async fn publish_conversation(&self, event: &DomainEvent) -> Result<(), ChatError> {
        match serde_json::to_string(event) {
            Ok(json) => tracing::debug!(event = %json, "conversation event (log publisher)"),
            Err(e) => tracing::warn!(error = %e, "failed to serialize conversation event"),
        }
        Ok(())
    }

    async fn publish_message(&self, event: &MessageEvent) -> Result<(), ChatError> {
        match serde_json::to_string(event) {
            Ok(json) => tracing::debug!(event = %json, "message event (log publisher)"),
            Err(e) => tracing::warn!(error = %e, "failed to serialize message event"),
        }
        Ok(())
    }
}
