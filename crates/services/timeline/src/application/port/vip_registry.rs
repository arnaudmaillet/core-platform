use async_trait::async_trait;

use crate::domain::aggregate::FeedEntry;
use crate::domain::value_object::{AuthorId, PostId};
use crate::error::TimelineError;

/// Port for the Redis VIP post registry: `timeline:vip:{author_id}` ZSETs.
///
/// Each VIP author has a single ZSET capped at `vip_registry_cap` entries,
/// scored by `published_at_ms`. On every feed read, the query handler
/// pipelines ZREVRANGEBYSCORE over each VIP followee's registry and merges
/// the results in-process with the caller's materialized feed.
///
/// VIP registries have a TTL (`vip_registry_ttl_secs`). The cold-start
/// hydration worker reconstructs them from `timeline.posts_by_author`.
#[async_trait]
pub trait VipRegistry: Send + Sync + 'static {
    /// Registers a VIP post in the author's Redis ZSET and enforces the cap.
    ///
    /// Also refreshes the ZSET TTL to `vip_registry_ttl_secs`.
    /// Atomically: ZADD + ZREMRANGEBYRANK + EXPIRE.
    async fn register(
        &self,
        entry:   &FeedEntry,
        cap:     u16,
        ttl_secs: u64,
    ) -> Result<(), TimelineError>;

    /// Removes a specific post from the VIP registry.
    async fn deregister(
        &self,
        author_id: &AuthorId,
        post_id:   &PostId,
    ) -> Result<(), TimelineError>;

    /// Returns at most `limit` most-recent entries for a VIP author.
    /// Used by the query handler to merge VIP content into the user's feed.
    async fn range_desc(
        &self,
        author_id:           &AuthorId,
        max_score_inclusive: i64,
        limit:               usize,
    ) -> Result<Vec<FeedEntry>, TimelineError>;

    /// Returns true if the `timeline:vip:{author_id}` key exists.
    async fn exists(&self, author_id: &AuthorId) -> Result<bool, TimelineError>;
}
