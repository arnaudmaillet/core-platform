use async_trait::async_trait;
use transport::error::TransportError;
use transport::kafka::envelope::EventEnvelope;
use transport::kafka::producer::handle::KafkaProducerHandle;

use crate::application::port::EventPublisher;
use crate::domain::event::DomainEvent;
use crate::error::MediaError;

use super::TOPIC_MEDIA_EVENTS;

/// Kafka-backed [`EventPublisher`]. Handlers persist durably first, then publish;
/// a publish failure surfaces as [`MediaError::EventPublishFailed`].
pub struct KafkaEventPublisher {
    producer: KafkaProducerHandle,
}

impl KafkaEventPublisher {
    pub fn new(producer: KafkaProducerHandle) -> Self {
        Self { producer }
    }
}

fn publish_err(e: TransportError) -> MediaError {
    MediaError::EventPublishFailed(e.to_string())
}

#[async_trait]
impl EventPublisher for KafkaEventPublisher {
    async fn publish(&self, event: &DomainEvent) -> Result<(), MediaError> {
        let key = event.asset_id().as_str();
        let envelope = EventEnvelope::new(TOPIC_MEDIA_EVENTS, key.clone(), event.clone())
            .with_header("event_type", event.event_type().to_owned())
            .with_header("asset_id", key);
        self.producer.publish(envelope).await.map_err(publish_err)
    }
}
