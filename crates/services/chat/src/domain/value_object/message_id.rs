use std::cmp::Ordering;
use std::fmt;
use uuid::Uuid;

use crate::error::ChatError;

/// Stable identity of a chat message.
///
/// Minted as a time-ordered UUIDv7 so identifiers sort chronologically by their
/// most-significant bytes. This single property is reused three ways:
/// - as the message's durable identity and natural dedupe key;
/// - as the clustering tail of the ScyllaDB message log (newest-first); and
/// - as the **public-since watermark cursor** on a conversation — when a private
///   group is toggled public, the watermark is the boundary `MessageId` and
///   guests may read only messages with `id >= public_since` (see
///   [`Conversation::publish`](crate::domain::aggregate::Conversation::publish)).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MessageId(Uuid);

impl MessageId {
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

    /// Unix-epoch milliseconds embedded in the UUIDv7. Used to translate the
    /// public-since watermark into the server-side history floor
    /// (`created_at >= ?`) for Audience-Plane reads. Returns `None` for a
    /// non-v7 UUID (e.g. a value reconstituted from legacy data).
    pub fn timestamp_ms(&self) -> Option<i64> {
        self.0.get_timestamp().map(|ts| {
            let (secs, nanos) = ts.to_unix();
            secs as i64 * 1_000 + (nanos as i64 / 1_000_000)
        })
    }
}

impl Default for MessageId {
    fn default() -> Self {
        Self::new()
    }
}

/// Ordered by the underlying UUIDv7 bytes, which are chronological. Lets a
/// `MessageId` act as a monotonic cursor for the public-since watermark and for
/// history pagination without a separate timestamp comparison.
impl PartialOrd for MessageId {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for MessageId {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.as_bytes().cmp(other.0.as_bytes())
    }
}

impl TryFrom<&str> for MessageId {
    type Error = ChatError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        Uuid::parse_str(s)
            .map(Self)
            .map_err(|_| ChatError::InvalidMessageId(s.to_owned()))
    }
}

impl fmt::Display for MessageId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
