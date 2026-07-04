use async_trait::async_trait;
use transport::{
    error::TransportError,
    kafka::{envelope::EventEnvelope, producer::handle::KafkaProducerHandle},
};

use crate::application::port::EngagementEventPublisher;
use crate::domain::event::reaction_event::ReactionKafkaEvent;
use crate::error::EngagementError;

const TOPIC_REACTIONS: &str = "engagement.reactions";

pub struct KafkaEngagementEventPublisher {
    producer: KafkaProducerHandle,
}

impl KafkaEngagementEventPublisher {
    pub fn new(producer: KafkaProducerHandle) -> Self {
        Self { producer }
    }
}

fn transport_err(e: TransportError) -> EngagementError {
    EngagementError::EventPublishFailed { message: e.to_string() }
}

#[async_trait]
impl EngagementEventPublisher for KafkaEngagementEventPublisher {
    async fn publish_reaction_event(&self, event: &ReactionKafkaEvent) -> Result<(), EngagementError> {
        let (post_id, profile_id, event_type) = match event {
            ReactionKafkaEvent::Upserted(e) => (e.post_id.as_str(), e.profile_id.as_str(), "upserted"),
            ReactionKafkaEvent::Removed(e)  => (e.post_id.as_str(), e.profile_id.as_str(), "removed"),
        };

        // Key by {post_id}:{profile_id} — all events for the same pair land on
        // the same Kafka partition, preserving ordering for the write-behind consumer.
        let key = format!("{}:{}", post_id, profile_id);

        let envelope = EventEnvelope::new(TOPIC_REACTIONS, key, event.clone())
            .with_header("event_type", event_type)
            .with_header("post_id",    post_id)
            .with_header("profile_id", profile_id);

        self.producer.publish(envelope).await.map_err(transport_err)
    }
}
