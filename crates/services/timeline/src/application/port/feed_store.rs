use async_trait::async_trait;

use crate::domain::aggregate::FeedEntry;
use crate::domain::value_object::{PostId, ProfileId};
use crate::error::TimelineError;

/// Port for the Redis hot feed layer: `timeline:feed:{profile_id}` ZSETs.
///
/// Each ZSET stores at most `feed_cap` `post_id` members scored by
/// `published_at_ms`. Entries are pruned oldest-first after each ZADD
/// via an atomic Lua cap script to bound memory usage per user.
///
/// This port is ONLY for regular (Standard/Premium) author posts.
/// VIP posts are managed via `VipRegistry`.
#[async_trait]
pub trait FeedStore: Send + Sync + 'static {
    /// Pushes a feed entry into the follower's Redis ZSET and enforces the cap.
    ///
    /// Atomically: ZADD + ZREMRANGEBYRANK if ZCARD > cap.
    /// Idempotent: re-adding the same post_id updates its score (same ms → no change).
    async fn push(
        &self,
        profile_id: &ProfileId,
        entry:      &FeedEntry,
        cap:        u16,
    ) -> Result<(), TimelineError>;

    /// Pushes a batch of entries into a follower's feed in a single pipeline.
    /// Used during follow-backfill to minimize round-trips.
    async fn push_batch(
        &self,
        profile_id: &ProfileId,
        entries:    &[FeedEntry],
        cap:        u16,
    ) -> Result<(), TimelineError>;

    /// Removes a specific post from a follower's feed.
    async fn remove_post(
        &self,
        profile_id: &ProfileId,
        post_id:    &PostId,
    ) -> Result<(), TimelineError>;

    /// Removes all posts authored by `author_id` from a follower's feed,
    /// identified by a list of post_ids obtained from ScyllaDB prune query.
    async fn remove_posts_batch(
        &self,
        profile_id: &ProfileId,
        post_ids:   &[PostId],
    ) -> Result<(), TimelineError>;

    /// Returns at most `limit` entries newer than `min_score_exclusive`
    /// (exclusive) sorted newest-first. Used by the cold-start hydration
    /// path to verify whether the ZSET is already populated.
    async fn range_desc(
        &self,
        profile_id:          &ProfileId,
        max_score_inclusive: i64,
        limit:               usize,
    ) -> Result<Vec<FeedEntry>, TimelineError>;

    /// Returns true if the `timeline:feed:{profile_id}` key exists in Redis.
    async fn exists(&self, profile_id: &ProfileId) -> Result<bool, TimelineError>;
}
