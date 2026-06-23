use std::fmt;
use uuid::Uuid;

use crate::error::ChatError;

/// Stable identity of a [`Conversation`](crate::domain::aggregate::Conversation).
///
/// Minted as a time-ordered UUIDv7. Doubles as the ScyllaDB message-log
/// partition base (`(conversation_id, bucket)`) and the Redis Cluster routing
/// key — both Member-Plane (`chat:{conv:<id>}:member`) and Audience-Plane
/// (`chat:{aud:<id>:<k>}`) channels are derived from it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ConversationId(Uuid);

impl ConversationId {
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }

    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    pub fn as_uuid(&self) -> Uuid {
        self.0
    }

    pub fn as_str(&self) -> String {
        self.0.to_string()
    }
}

impl Default for ConversationId {
    fn default() -> Self {
        Self::new()
    }
}

impl TryFrom<&str> for ConversationId {
    type Error = ChatError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        Uuid::parse_str(s)
            .map(Self)
            .map_err(|_| ChatError::InvalidConversationId(s.to_owned()))
    }
}

impl fmt::Display for ConversationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
