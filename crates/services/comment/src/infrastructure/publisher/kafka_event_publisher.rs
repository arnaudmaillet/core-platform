use async_trait::async_trait;
use transport::{
    error::TransportError,
    kafka::{envelope::EventEnvelope, producer::handle::KafkaProducerHandle},
};

use crate::application::port::CommentEventPublisher;
use crate::domain::event::{CommentCreatedEvent, CommentDeletedEvent, DomainEvent};
use crate::error::CommentError;

const TOPIC_CREATED: &str = "comment.created";
const TOPIC_DELETED: &str = "comment.deleted";

fn transport_err(e: TransportError) -> CommentError {
    CommentError::EventPublishFailed { message: e.to_string() }
}

pub struct KafkaCommentEventPublisher {
    producer: KafkaProducerHandle,
}

impl KafkaCommentEventPublisher {
    pub fn new(producer: KafkaProducerHandle) -> Self {
        Self { producer }
    }
}

#[async_trait]
impl CommentEventPublisher for KafkaCommentEventPublisher {
    async fn publish(&self, event: &DomainEvent) -> Result<(), CommentError> {
        match event {
            DomainEvent::CommentCreated(e) => publish_created(&self.producer, e).await,
            DomainEvent::CommentDeleted(e) => publish_deleted(&self.producer, e).await,
        }
    }
}

async fn publish_created(
    producer: &KafkaProducerHandle,
    event:    &CommentCreatedEvent,
) -> Result<(), CommentError> {
    let key = event.comment_id.clone();
    let envelope = EventEnvelope::new(TOPIC_CREATED, key, event.clone())
        .with_header("event_type",  "CommentCreated")
        .with_header("comment_id",  event.comment_id.as_str())
        .with_header("post_id",     event.post_id.as_str())
        .with_header("author_id",   event.author_id.as_str());

    producer.publish(envelope).await.map_err(transport_err)
}

async fn publish_deleted(
    producer: &KafkaProducerHandle,
    event:    &CommentDeletedEvent,
) -> Result<(), CommentError> {
    let key = event.comment_id.clone();
    let envelope = EventEnvelope::new(TOPIC_DELETED, key, event.clone())
        .with_header("event_type",  "CommentDeleted")
        .with_header("comment_id",  event.comment_id.as_str())
        .with_header("post_id",     event.post_id.as_str())
        .with_header("author_id",   event.author_id.as_str());

    producer.publish(envelope).await.map_err(transport_err)
}
