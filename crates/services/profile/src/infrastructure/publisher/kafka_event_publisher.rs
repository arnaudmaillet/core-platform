use async_trait::async_trait;
use transport::error::TransportError;
use transport::kafka::envelope::EventEnvelope;
use transport::kafka::producer::handle::KafkaProducerHandle;

use super::wire::ProfileEventWire;
use crate::application::port::EventPublisher;
use crate::domain::event::DomainEvent;
use crate::error::ProfileError;

/// The single versioned topic carrying every profile lifecycle event
/// (moderation-service convention), keyed by `profile_id`.
const TOPIC: &str = "profile.v1.events";

fn transport_err(e: TransportError) -> ProfileError {
    ProfileError::DomainViolation {
        field: "event_publish".to_owned(),
        message: e.to_string(),
    }
}

/// Publishes profile domain events to `profile.v1.events`.
pub struct KafkaProfileEventPublisher {
    producer: KafkaProducerHandle,
}

impl KafkaProfileEventPublisher {
    pub fn new(producer: KafkaProducerHandle) -> Self {
        Self { producer }
    }
}

#[async_trait]
impl EventPublisher for KafkaProfileEventPublisher {
    async fn publish(&self, event: &DomainEvent) -> Result<(), ProfileError> {
        let wire = ProfileEventWire::from(event);
        let key = wire.profile_id().to_owned();
        let event_type = wire.event_type();
        let envelope = EventEnvelope::new(TOPIC, key, wire).with_header("event_type", event_type);
        self.producer.publish(envelope).await.map_err(transport_err)
    }
}
