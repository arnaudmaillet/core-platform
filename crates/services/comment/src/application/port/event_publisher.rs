use async_trait::async_trait;

use crate::{domain::event::DomainEvent, error::CommentError};

/// Port for publishing comment domain events to Kafka.
///
/// `CommentCreated` is keyed by `comment_id` and published to `comment.created`.
/// `CommentDeleted` is keyed by `comment_id` and published to `comment.deleted`.
///
/// The engagement service's `CommentEventConsumer` subscribes to both topics
/// to drive its atomic Redis and ScyllaDB comment counters.
#[async_trait]
pub trait CommentEventPublisher: Send + Sync + 'static {
    async fn publish(&self, event: &DomainEvent) -> Result<(), CommentError>;
}
