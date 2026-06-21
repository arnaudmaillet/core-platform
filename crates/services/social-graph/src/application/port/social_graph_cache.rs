use async_trait::async_trait;

use crate::domain::value_object::ProfileId;
use crate::error::SocialGraphError;

/// Follower and following counts for a single profile, read from Redis.
///
/// These counters are the only counts maintained in-process.
/// ScyllaDB COUNT(*) over large partitions is prohibitively expensive for
/// high-traffic profiles — Redis INCR/DECR is the authoritative counter store.
#[derive(Debug, Clone, Default)]
pub struct RelationCounts {
    /// Total number of profiles following this profile.
    pub followers: i64,
    /// Total number of profiles this profile follows.
    pub following: i64,
}

/// Redis cache port for the social graph.
///
/// # Key namespace
///
/// | Key pattern                           | Type   | Purpose                         |
/// |---------------------------------------|--------|---------------------------------|
/// | `sg:following:v1:{profile_id}`        | Set    | UUIDs of profiles this ID follows|
/// | `sg:blocks:v1:{profile_id}`           | Set    | UUIDs this ID has blocked       |
/// | `sg:followers_count:v1:{profile_id}`  | String | Follower counter (INCR/DECR)    |
/// | `sg:following_count:v1:{profile_id}`  | String | Following counter (INCR/DECR)   |
///
/// # Why Sets for `following` but Strings for `followers`
///
/// Outbound follows are bounded for all normal users (max tens of thousands).
/// Inbound follows are unbounded for celebrities (millions). Materialising the
/// full inbound set in Redis would exhaust memory on high-follower accounts.
/// A counter satisfies the read need with O(1) space.
///
/// # Cache miss semantics
///
/// All methods are best-effort: callers must not treat a cache error as a
/// domain error. Failures are logged and swallowed at the handler level so
/// a Redis outage degrades to slower ScyllaDB reads, never a user-visible error.
#[async_trait]
pub trait SocialGraphCache: Send + Sync + 'static {
    // ── Following set operations ──────────────────────────────────────────────

    /// SADD `sg:following:v1:{follower}` `{followee}`.
    async fn add_following(
        &self,
        follower_id: &ProfileId,
        followee_id: &ProfileId,
    ) -> Result<(), SocialGraphError>;

    /// SREM `sg:following:v1:{follower}` `{followee}`.
    async fn remove_following(
        &self,
        follower_id: &ProfileId,
        followee_id: &ProfileId,
    ) -> Result<(), SocialGraphError>;

    // ── Block set operations ──────────────────────────────────────────────────

    /// SADD `sg:blocks:v1:{blocker}` `{blockee}`.
    async fn add_block(
        &self,
        blocker_id: &ProfileId,
        blockee_id: &ProfileId,
    ) -> Result<(), SocialGraphError>;

    /// SREM `sg:blocks:v1:{blocker}` `{blockee}`.
    async fn remove_block(
        &self,
        blocker_id: &ProfileId,
        blockee_id: &ProfileId,
    ) -> Result<(), SocialGraphError>;

    // ── Counter operations ────────────────────────────────────────────────────

    /// INCR `sg:followers_count:v1:{profile_id}`.
    async fn incr_followers_count(&self, profile_id: &ProfileId) -> Result<(), SocialGraphError>;

    /// DECR `sg:followers_count:v1:{profile_id}` (floor at 0).
    async fn decr_followers_count(&self, profile_id: &ProfileId) -> Result<(), SocialGraphError>;

    /// INCR `sg:following_count:v1:{profile_id}`.
    async fn incr_following_count(&self, profile_id: &ProfileId) -> Result<(), SocialGraphError>;

    /// DECR `sg:following_count:v1:{profile_id}` (floor at 0).
    async fn decr_following_count(&self, profile_id: &ProfileId) -> Result<(), SocialGraphError>;

    /// GET both counter keys for a profile in a single call.
    ///
    /// Returns zero for keys that do not exist (cold cache after Redis restart).
    async fn get_counts(&self, profile_id: &ProfileId) -> Result<RelationCounts, SocialGraphError>;
}
