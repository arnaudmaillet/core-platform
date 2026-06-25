use std::sync::Arc;

use async_trait::async_trait;
use tracing::warn;

use crate::application::port::EventPublisher;
use crate::domain::event::DomainEvent;
use crate::error::ModerationError;

/// Fans a domain event out to one **authoritative** sink (Kafka — the Plane B
/// denormalization notification, whose failure must surface) and zero or more
/// **best-effort** sinks (the Scylla evidence history — an audit projection whose
/// transient failure must not fail the moderation operation, only be logged).
pub struct FanoutEventPublisher {
    primary: Arc<dyn EventPublisher>,
    secondaries: Vec<Arc<dyn EventPublisher>>,
}

impl FanoutEventPublisher {
    pub fn new(primary: Arc<dyn EventPublisher>, secondaries: Vec<Arc<dyn EventPublisher>>) -> Self {
        Self { primary, secondaries }
    }
}

#[async_trait]
impl EventPublisher for FanoutEventPublisher {
    async fn publish(&self, event: &DomainEvent) -> Result<(), ModerationError> {
        for sink in &self.secondaries {
            if let Err(e) = sink.publish(event).await {
                warn!(event_type = event.event_type(), error = %e, "best-effort event sink failed");
            }
        }
        self.primary.publish(event).await
    }
}
