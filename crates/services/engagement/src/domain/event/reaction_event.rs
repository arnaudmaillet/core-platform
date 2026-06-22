use serde::{Deserialize, Serialize};

use crate::domain::value_object::ReactionKind;

/// Emitted when a profile adds or replaces its reaction on a post.
///
/// Published to Kafka topic `engagement.reactions` (key: `{post_id}:{profile_id}`)
/// for the write-behind worker to durably persist to ScyllaDB.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReactionUpsertedEvent {
    pub post_id:    String,
    pub profile_id: String,
    pub new_kind:   ReactionKind,
    pub new_weight: i64,
    /// The previous reaction, if one existed. Drives the ScyllaDB counter delta.
    pub old_kind:   Option<ReactionKind>,
    pub old_weight: Option<i64>,
    pub event_at_ms: i64,
}

/// Emitted when a profile explicitly removes its reaction from a post.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReactionRemovedEvent {
    pub post_id:    String,
    pub profile_id: String,
    pub kind:       ReactionKind,
    pub weight:     i64,
    pub event_at_ms: i64,
}

/// Discriminated union published to the single `engagement.reactions` Kafka topic.
/// The `event_type` tag drives write-behind consumer routing.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event_type", rename_all = "snake_case")]
pub enum ReactionKafkaEvent {
    Upserted(ReactionUpsertedEvent),
    Removed(ReactionRemovedEvent),
}
