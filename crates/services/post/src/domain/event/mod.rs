use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostPublishedEvent {
    pub post_id:         String,
    pub profile_id:      String,
    pub kind:            String,
    pub published_at_ms: i64,
    /// The author's tier at publish time (0=Standard, 1=Premium, 2=Vip),
    /// denormalized from `profile.v1.events` and stamped by the publish handler.
    /// `timeline` routes VIP authors to its read path on this field.
    #[serde(default)]
    pub author_tier:     u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio_id:        Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio_kind:      Option<u8>,
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
