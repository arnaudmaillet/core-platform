use async_trait::async_trait;
use transport::error::TransportError;
use transport::kafka::envelope::EventEnvelope;
use transport::kafka::producer::handle::KafkaProducerHandle;

use crate::application::port::EventPublisher;
use crate::domain::event::DomainEvent;
use crate::error::AuthError;

use super::{event_key, TOPIC_AUTH_EVENTS};

/// Kafka-backed [`EventPublisher`]. Handlers persist durably first, then publish;
/// a publish failure surfaces as [`AuthError::EventPublishFailed`] (the adapter
/// may itself back an outbox in a later iteration).
pub struct KafkaEventPublisher {
    producer: KafkaProducerHandle,
}

impl KafkaEventPublisher {
    pub fn new(producer: KafkaProducerHandle) -> Self {
        Self { producer }
    }
}

fn publish_err(e: TransportError) -> AuthError {
    AuthError::EventPublishFailed(e.to_string())
}

#[async_trait]
impl EventPublisher for KafkaEventPublisher {
    async fn publish(&self, event: &DomainEvent) -> Result<(), AuthError> {
        let key = event_key(event).as_str();
        let envelope = EventEnvelope::new(TOPIC_AUTH_EVENTS, key.clone(), event.clone())
            .with_header("event_type", event.event_type().to_owned())
            .with_header("account_id", key);
        self.producer.publish(envelope).await.map_err(publish_err)
    }
}
