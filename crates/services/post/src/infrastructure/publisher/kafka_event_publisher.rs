use async_trait::async_trait;
use transport::{
    error::TransportError,
    kafka::{envelope::EventEnvelope, producer::handle::KafkaProducerHandle},
};

use crate::application::port::EventPublisher;
use crate::domain::event::{DomainEvent, PostDeletedEvent, PostPublishedEvent, PostUpdatedEvent};
use crate::error::PostError;

// Legacy per-type topics (bare payloads), consumed by notification / geo-discovery
// / timeline. Retained for backward compatibility.
const TOPIC_PUBLISHED: &str = "post.published";
const TOPIC_UPDATED:   &str = "post.updated";
const TOPIC_DELETED:   &str = "post.deleted";

// The unified, versioned stream (the fleet convention, like `moderation.v1.events`
// / `profile.v1.events`): the whole internally-tagged `DomainEvent`, keyed by
// `post_id`. Consumed by `search`. Published ALONGSIDE the legacy topics so the
// existing per-type consumers keep working.
const TOPIC_V1: &str = "post.v1.events";

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
        // 1. Legacy per-type topics (existing consumers).
        match event {
            DomainEvent::PostPublished(e) => publish_published(&self.producer, e).await?,
            DomainEvent::PostUpdated(e)   => publish_updated(&self.producer, e).await?,
            DomainEvent::PostDeleted(e)   => publish_deleted(&self.producer, e).await?,
        }
        // 2. Unified `post.v1.events` stream (the fleet convention; consumed by search).
        publish_v1(&self.producer, event).await
    }
}

/// Publishes the whole internally-tagged event to `post.v1.events`, keyed by
/// `post_id`. `DomainEvent` is `#[serde(tag = "type")]`, so the wire payload is
/// `{"type":"PostPublished", ...}` — the shape `search` deserializes.
async fn publish_v1(
    producer: &KafkaProducerHandle,
    event:    &DomainEvent,
) -> Result<(), PostError> {
    let (post_id, profile_id, event_type) = match event {
        DomainEvent::PostPublished(e) => (&e.post_id, &e.profile_id, "PostPublished"),
        DomainEvent::PostUpdated(e)   => (&e.post_id, &e.profile_id, "PostUpdated"),
        DomainEvent::PostDeleted(e)   => (&e.post_id, &e.profile_id, "PostDeleted"),
    };
    let envelope = EventEnvelope::new(TOPIC_V1, post_id.clone(), event.clone())
        .with_header("event_type", event_type)
        .with_header("post_id",    post_id.clone())
        .with_header("profile_id", profile_id.clone());

    producer.publish(envelope).await.map_err(transport_err)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::event::PostPublishedEvent;

    /// Locks the `post.v1.events` wire shape the `search` decoder depends on:
    /// internally tagged on `type`, with the fields flattened alongside it.
    #[test]
    fn post_v1_payload_is_internally_tagged() {
        let event = DomainEvent::PostPublished(PostPublishedEvent {
            post_id:         "post-1".to_owned(),
            profile_id:      "prof-9".to_owned(),
            kind:            "text".to_owned(),
            published_at_ms: 1_700_000_000_000,
            audio_id:        None,
            audio_kind:      None,
        });
        let value = serde_json::to_value(&event).expect("serialize");
        assert_eq!(value["type"], "PostPublished");
        assert_eq!(value["post_id"], "post-1");
        assert_eq!(value["profile_id"], "prof-9");
        assert_eq!(value["published_at_ms"], 1_700_000_000_000_i64);
    }
}
