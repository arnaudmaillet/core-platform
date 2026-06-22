use async_trait::async_trait;
use transport::{
    error::TransportError,
    kafka::{envelope::EventEnvelope, producer::handle::KafkaProducerHandle},
};

use crate::application::port::EventPublisher;
use crate::domain::event::{DomainEvent, PostDeletedEvent, PostPublishedEvent, PostUpdatedEvent};
use crate::error::PostError;

const TOPIC_PUBLISHED: &str = "post.published";
const TOPIC_UPDATED:   &str = "post.updated";
const TOPIC_DELETED:   &str = "post.deleted";

fn transport_err(e: TransportError) -> PostError {
    PostError::DomainViolation {
        field:   "kafka".to_owned(),
        message: e.to_string(),
    }
}

pub struct KafkaEventPublisher {
    producer: KafkaProducerHandle,
}

impl KafkaEventPublisher {
    pub fn new(producer: KafkaProducerHandle) -> Self {
        Self { producer }
    }
}

#[async_trait]
impl EventPublisher for KafkaEventPublisher {
    async fn publish(&self, event: &DomainEvent) -> Result<(), PostError> {
        match event {
            DomainEvent::PostPublished(e) => publish_published(&self.producer, e).await,
            DomainEvent::PostUpdated(e)   => publish_updated(&self.producer, e).await,
            DomainEvent::PostDeleted(e)   => publish_deleted(&self.producer, e).await,
        }
    }
}

async fn publish_published(
    producer: &KafkaProducerHandle,
    event:    &PostPublishedEvent,
) -> Result<(), PostError> {
    let key      = event.post_id.clone();
    let envelope = EventEnvelope::new(TOPIC_PUBLISHED, key, event.clone())
        .with_header("event_type",  "PostPublished")
        .with_header("post_id",     event.post_id.clone())
        .with_header("profile_id",  event.profile_id.clone());

    producer.publish(envelope).await.map_err(transport_err)
}

async fn publish_updated(
    producer: &KafkaProducerHandle,
    event:    &PostUpdatedEvent,
) -> Result<(), PostError> {
    let key      = event.post_id.clone();
    let envelope = EventEnvelope::new(TOPIC_UPDATED, key, event.clone())
        .with_header("event_type",  "PostUpdated")
        .with_header("post_id",     event.post_id.clone())
        .with_header("profile_id",  event.profile_id.clone());

    producer.publish(envelope).await.map_err(transport_err)
}

async fn publish_deleted(
    producer: &KafkaProducerHandle,
    event:    &PostDeletedEvent,
) -> Result<(), PostError> {
    let key      = event.post_id.clone();
    let envelope = EventEnvelope::new(TOPIC_DELETED, key, event.clone())
        .with_header("event_type",  "PostDeleted")
        .with_header("post_id",     event.post_id.clone())
        .with_header("profile_id",  event.profile_id.clone());

    producer.publish(envelope).await.map_err(transport_err)
}
