use chrono::{DateTime, Utc};

use crate::domain::value_object::{AuthorTier, ProfileId};

/// Emitted when a profile's follower count crosses a tier boundary. This is the
/// fleet's author-tier **signal** — consumed by `profile` (which persists the tier
/// and re-emits it on `profile.v1.events` for `post` to denormalize). The wire
/// payload renders `new_tier` as its `u8` taxonomy value (see the Kafka publisher).
#[derive(Debug, Clone)]
pub struct AuthorTierChanged {
    /// The author whose tier changed (the followee whose follower count moved).
    pub profile_id: ProfileId,
    pub new_tier: AuthorTier,
    /// The follower count at the moment of the crossing.
    pub follower_count: i64,
    pub changed_at: DateTime<Utc>,
}
