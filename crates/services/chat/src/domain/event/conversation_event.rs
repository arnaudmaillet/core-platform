use serde::{Deserialize, Serialize};

/// Lifecycle events emitted by the [`Conversation`](crate::domain::aggregate::Conversation)
/// aggregate as it mutates.
///
/// Buffered on the aggregate (`pending_events`) and drained via
/// `take_events()` after a successful transition, mirroring the `post` service.
/// The infrastructure layer publishes the drained events to Kafka so downstream
/// consumers (audience fan-out wiring, subscriber notification, cache
/// invalidation) react to `Published` / `Unpublished` toggles.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event_type", rename_all = "snake_case")]
pub enum DomainEvent {
    ConversationCreated(ConversationCreatedEvent),
    ConversationPublished(ConversationPublishedEvent),
    ConversationUnpublished(ConversationUnpublishedEvent),
    MemberJoined(MemberJoinedEvent),
    MemberLeft(MemberLeftEvent),
}

/// Emitted when a conversation is created. A `Channel` is born public; a `Group`
/// is born private.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationCreatedEvent {
    pub conversation_id: String,
    pub kind:            String,
    pub visibility:      String,
    pub owner_id:        String,
    pub created_at_ms:   i64,
}

/// Emitted on a `Private -> Public` toggle. `public_since` is the watermark
/// `MessageId`: audience members may read only messages with `id >= public_since`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationPublishedEvent {
    pub conversation_id: String,
    pub public_since:    String,
    pub published_at_ms: i64,
}

/// Emitted on a `Public -> Private` toggle. Signals the infrastructure layer to
/// tear down the Audience Plane and cancel live guest streams.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationUnpublishedEvent {
    pub conversation_id:   String,
    pub unpublished_at_ms: i64,
}

/// Emitted when a profile is admitted to the bounded Member Plane.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemberJoinedEvent {
    pub conversation_id: String,
    pub profile_id:      String,
    pub role:            String,
    pub joined_at_ms:    i64,
}

/// Emitted when a profile leaves (or is removed from) the Member Plane.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemberLeftEvent {
    pub conversation_id: String,
    pub profile_id:      String,
    pub left_at_ms:      i64,
}
