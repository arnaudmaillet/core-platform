use chrono::{DateTime, Utc};

use crate::domain::value_object::ProfileId;

/// A single directed follow edge as returned by adjacency-list queries.
///
/// In `list_followers` results: `profile_id` is the follower.
/// In `list_following` results: `profile_id` is the followee.
#[derive(Debug, Clone)]
pub struct FollowEdge {
    pub profile_id:  ProfileId,
    pub followed_at: DateTime<Utc>,
}
