use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostPublishedEvent {
    pub post_id:    String,
    pub profile_id: String,
    pub kind:       String,
    pub published_at_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostUpdatedEvent {
    pub post_id:    String,
    pub profile_id: String,
    pub updated_at_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostDeletedEvent {
    pub post_id:    String,
    pub profile_id: String,
    pub deleted_at_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum DomainEvent {
    PostPublished(PostPublishedEvent),
    PostUpdated(PostUpdatedEvent),
    PostDeleted(PostDeletedEvent),
}
