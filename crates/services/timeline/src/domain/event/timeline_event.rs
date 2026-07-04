use crate::domain::aggregate::FeedEntry;
use crate::domain::value_object::{PostId, ProfileId};

/// Domain events emitted by the timeline service during write operations.
///
/// These events are internal to the service — timeline does not publish
/// to Kafka. They exist to capture state transitions for local observers
/// (telemetry, in-process assertions in tests).
#[derive(Debug, Clone)]
pub enum TimelineEvent {
    /// A post was added to a follower's feed (fan-out-on-write path).
    FeedItemIngested {
        entry:      FeedEntry,
        profile_id: ProfileId,
    },
    /// A post was removed from a follower's feed.
    FeedItemRemoved {
        post_id:    PostId,
        profile_id: ProfileId,
    },
    /// A post was registered in a VIP author's ZSET registry (fan-out-on-read path).
    VipPostRegistered {
        entry: FeedEntry,
    },
    /// A post was removed from a VIP author's ZSET registry.
    VipPostDeregistered {
        post_id:   PostId,
        author_id: crate::domain::value_object::AuthorId,
    },
}
