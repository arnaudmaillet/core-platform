use async_trait::async_trait;

use crate::domain::aggregate::FeedEntry;
use crate::domain::value_object::{AuthorId, PostId, ProfileId};
use crate::error::TimelineError;

/// Port for the ScyllaDB durable feed layer: `timeline.feed_items_by_profile`.
///
/// This table is the cold-start source for regular-author feed items.
/// All hot-path reads are served by Redis. This port is called:
///   - On write: fan-out INSERT per follower (background, fire-and-forget)
///   - On cold-start: range scan per profile to rebuild Redis ZSET
///   - On post.deleted: DELETE per post_id (best-effort, background)
///   - On follow.deleted: range scan + DELETE per author_id in a partition
#[async_trait]
pub trait FeedRepository: Send + Sync + 'static {
    /// Inserts one materialized feed entry for a follower. Idempotent (LWW).
    async fn insert(
        &self,
        profile_id: &ProfileId,
        entry:      &FeedEntry,
    ) -> Result<(), TimelineError>;

    /// Batch-inserts a list of entries for a follower. Used during backfill.
    async fn insert_batch(
        &self,
        profile_id: &ProfileId,
        entries:    &[FeedEntry],
    ) -> Result<(), TimelineError>;

    /// Reads at most `limit` most-recent feed entries for a profile.
    /// Used during Redis cold-start to rebuild `timeline:feed:{profile_id}`.
    ///
    /// Cursor semantics: pass `before_ms = i64::MAX` for the first page,
    /// then pass the smallest `published_at_ms` from the previous page.
    async fn list_recent(
        &self,
        profile_id: &ProfileId,
        before_ms:  i64,
        limit:      i32,
    ) -> Result<Vec<FeedEntry>, TimelineError>;

    /// Deletes a specific (profile, post) entry. Used on post.deleted for
    /// followers of Standard/Premium authors. Idempotent (no-op if absent).
    async fn delete(
        &self,
        profile_id: &ProfileId,
        post_id:    &PostId,
        published_at_ms: i64,
    ) -> Result<(), TimelineError>;

    /// Returns all post_ids authored by `author_id` in a follower's feed partition.
    /// Used during follow.deleted to collect post_ids for Redis ZREM.
    async fn list_by_author(
        &self,
        profile_id: &ProfileId,
        author_id:  &AuthorId,
    ) -> Result<Vec<(PostId, i64)>, TimelineError>;
}
