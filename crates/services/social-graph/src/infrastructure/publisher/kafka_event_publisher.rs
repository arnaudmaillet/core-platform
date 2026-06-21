use async_trait::async_trait;
use transport::{
    error::TransportError,
    kafka::{envelope::EventEnvelope, producer::handle::KafkaProducerHandle},
};

use crate::application::port::EventPublisher;
use crate::domain::event::{DomainEvent, ProfileBlocked, ProfileFollowed, ProfileUnfollowed};
use crate::error::SocialGraphError;

const TOPIC_FOLLOWED:   &str = "social-graph.followed";
const TOPIC_UNFOLLOWED: &str = "social-graph.unfollowed";
const TOPIC_BLOCKED:    &str = "social-graph.blocked";

fn transport_err(e: TransportError) -> SocialGraphError {
    SocialGraphError::DomainViolation {
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
    async fn publish(&self, event: &DomainEvent) -> Result<(), SocialGraphError> {
        match event {
            DomainEvent::ProfileFollowed(e) => publish_followed(&self.producer, e).await,
            DomainEvent::ProfileUnfollowed(e) => publish_unfollowed(&self.producer, e).await,
            DomainEvent::ProfileBlocked(e) => publish_blocked(&self.producer, e).await,
            // ProfileUnblocked is not published downstream per the interface contract.
            DomainEvent::ProfileUnblocked(_) => Ok(()),
        }
    }
}

async fn publish_followed(
    producer: &KafkaProducerHandle,
    event:    &ProfileFollowed,
) -> Result<(), SocialGraphError> {
    let key      = format!("{}:{}", event.actor_id, event.target_id);
    let envelope = EventEnvelope::new(TOPIC_FOLLOWED, key, event.clone())
        .with_header("event_type", "ProfileFollowed")
        .with_header("actor_id",   event.actor_id.as_str())
        .with_header("target_id",  event.target_id.as_str());

    producer.publish(envelope).await.map_err(transport_err)
}

async fn publish_unfollowed(
    producer: &KafkaProducerHandle,
    event:    &ProfileUnfollowed,
) -> Result<(), SocialGraphError> {
    let key      = format!("{}:{}", event.actor_id, event.target_id);
    let envelope = EventEnvelope::new(TOPIC_UNFOLLOWED, key, event.clone())
        .with_header("event_type", "ProfileUnfollowed")
        .with_header("actor_id",   event.actor_id.as_str())
        .with_header("target_id",  event.target_id.as_str());

    producer.publish(envelope).await.map_err(transport_err)
}

async fn publish_blocked(
    producer: &KafkaProducerHandle,
    event:    &ProfileBlocked,
) -> Result<(), SocialGraphError> {
    let key      = format!("{}:{}", event.actor_id, event.target_id);
    let envelope = EventEnvelope::new(TOPIC_BLOCKED, key, event.clone())
        .with_header("event_type", "ProfileBlocked")
        .with_header("actor_id",   event.actor_id.as_str())
        .with_header("target_id",  event.target_id.as_str());

    producer.publish(envelope).await.map_err(transport_err)
}
