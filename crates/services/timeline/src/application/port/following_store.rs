use async_trait::async_trait;

use crate::domain::value_object::{AuthorId, ProfileId};
use crate::error::TimelineError;

/// Port for the Redis following-set cache: `timeline:following:{profile_id}`.
///
/// Stores the set of author UUIDs that a profile follows, maintained by
/// `FollowCreatedWorker` and `FollowDeletedWorker`. Used by `GetFollowingFeedQuery`
/// to split the following list into VIP vs regular at read-time.
///
/// No TTL — the SET is permanent until the profile unfollows all authors.
/// Cold-start recovery: if the key is absent, the query handler calls
/// `SocialGraphClient.list_all_following` to rebuild it.
#[async_trait]
pub trait FollowingStore: Send + Sync + 'static {
    /// Adds a followee to the profile's following set.
    async fn add(
        &self,
        follower_id: &ProfileId,
        followee_id: &AuthorId,
    ) -> Result<(), TimelineError>;

    /// Removes a followee from the profile's following set.
    async fn remove(
        &self,
        follower_id: &ProfileId,
        followee_id: &AuthorId,
    ) -> Result<(), TimelineError>;

    /// Returns all followee IDs for a profile.
    /// Returns an empty Vec if the key does not exist (cold state).
    async fn get_all(
        &self,
        follower_id: &ProfileId,
    ) -> Result<Vec<AuthorId>, TimelineError>;

    /// Returns true if the `timeline:following:{profile_id}` key exists.
    /// Used to distinguish "no followees" from "cold cache".
    async fn exists(&self, follower_id: &ProfileId) -> Result<bool, TimelineError>;

    /// Bulk-populates the following set from a list of followees.
    /// Used during cold-start rebuild from the social-graph service.
    async fn set_all(
        &self,
        follower_id: &ProfileId,
        followee_ids: &[AuthorId],
    ) -> Result<(), TimelineError>;
}
