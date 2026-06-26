use async_trait::async_trait;
use transport::error::TransportError;
use transport::kafka::envelope::EventEnvelope;
use transport::kafka::producer::handle::KafkaProducerHandle;

use crate::application::port::EventPublisher;
use crate::domain::event::DomainEvent;
use crate::error::AccountError;

use super::{TOPIC_ACCOUNT_EVENTS, event_key};

/// Kafka-backed [`EventPublisher`]. The repository persists durably first, then
/// publishes; a publish failure surfaces as [`AccountError::EventPublishFailed`].
pub struct KafkaEventPublisher {
    producer: KafkaProducerHandle,
}

impl KafkaEventPublisher {
    pub fn new(producer: KafkaProducerHandle) -> Self {
        Self { producer }
    }
}

fn publish_err(e: TransportError) -> AccountError {
    AccountError::EventPublishFailed(e.to_string())
}

#[async_trait]
impl EventPublisher for KafkaEventPublisher {
    async fn publish(&self, event: &DomainEvent) -> Result<(), AccountError> {
        let key = event_key(event).as_str();
        let envelope = EventEnvelope::new(TOPIC_ACCOUNT_EVENTS, key.clone(), event.clone())
            .with_header("event_type", event.event_type().to_owned())
            .with_header("account_id", key);
        self.producer.publish(envelope).await.map_err(publish_err)
    }
}
