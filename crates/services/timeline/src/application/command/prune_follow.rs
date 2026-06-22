use std::sync::Arc;

use cqrs::{Command, CommandHandler, Envelope};
use validate_core::{FieldViolation, Validate};

use crate::application::port::{
    FeedRepository, FeedStore, FollowingStore, TierCache,
};
use crate::domain::value_object::{AuthorId, AuthorTier, FanOutMode, ProfileId};
use crate::error::TimelineError;

/// Triggered by `FollowDeletedWorker` when a `social-graph.unfollowed` event arrives.
///
/// Prune strategy:
///   Standard/Premium followee:
///     1. Query `feed_items_by_profile` for all post_ids authored by the unfollowed
///        author in the follower's ScyllaDB partition (full-partition scan filtered
///        by author_id in application code — acceptable since partition is per-follower).
///     2. Pipeline ZREM from the follower's `timeline:feed:{follower_id}` ZSET.
///     3. Background DELETE each row from ScyllaDB `feed_items_by_profile`.
///   Vip followee:
///     NO-OP — VIP posts are not stored in the follower's feed; they are merged at
///     read-time from `timeline:vip:{author_id}`. The query handler will exclude VIP
///     authors not in `timeline:following:{follower_id}` automatically.
///
/// Also removes the followee from `timeline:following:{follower_id}` so the read-path
/// no longer includes them in VIP merges.
pub struct PruneFollowCommand {
    /// The profile that performed the unfollow action.
    pub follower_id: String,
    /// The profile that was unfollowed.
    pub followee_id: String,
}

impl Command for PruneFollowCommand {}

impl Validate for PruneFollowCommand {
    fn validate(&self) -> Result<(), Vec<FieldViolation>> {
        let mut v = Vec::new();
        if self.follower_id.trim().is_empty() {
            v.push(FieldViolation::new("follower_id", "TML-VAL-030", "follower_id must not be empty"));
        }
        if self.followee_id.trim().is_empty() {
            v.push(FieldViolation::new("followee_id", "TML-VAL-031", "followee_id must not be empty"));
        }
        if v.is_empty() { Ok(()) } else { Err(v) }
    }
}

pub struct PruneFollowHandler<FS, FR, TC, FO> {
    pub feed_store:      Arc<FS>,
    pub feed_repository: Arc<FR>,
    pub tier_cache:      Arc<TC>,
    pub following_store: Arc<FO>,
}

impl<FS, FR, TC, FO> CommandHandler<PruneFollowCommand>
    for PruneFollowHandler<FS, FR, TC, FO>
where
    FS: FeedStore,
    FR: FeedRepository,
    TC: TierCache,
    FO: FollowingStore,
{
    type Error = TimelineError;

    async fn handle(
        &self,
        envelope: Envelope<PruneFollowCommand>,
    ) -> Result<(), TimelineError> {
        let cmd = &envelope.payload;

        let follower_id = ProfileId::try_from(cmd.follower_id.as_str())?;
        let followee_id = AuthorId::try_from(cmd.followee_id.as_str())?;

        // Remove from Redis following set immediately.
        self.following_store.remove(&follower_id, &followee_id).await?;

        let tier = self
            .tier_cache
            .get_tier(&followee_id)
            .await?
            .unwrap_or(AuthorTier::Standard);

        if matches!(tier.fan_out_mode(), FanOutMode::Read) {
            // VIP: following_store removal is sufficient — VIP ZSET is per-author.
            tracing::debug!(
                follower_id = %follower_id,
                followee_id = %followee_id,
                "VIP unfollow — following_store updated; no feed prune needed"
            );
            return Ok(());
        }

        // Standard/Premium: collect post_ids from ScyllaDB and ZREM from Redis.
        let authored_posts = self
            .feed_repository
            .list_by_author(&follower_id, &followee_id)
            .await?;

        if authored_posts.is_empty() {
            return Ok(());
        }

        let post_ids: Vec<_> = authored_posts.iter().map(|(pid, _)| *pid).collect();

        // Redis prune (best-effort, eventual consistency for any cache miss).
        if let Err(e) = self
            .feed_store
            .remove_posts_batch(&follower_id, &post_ids)
            .await
        {
            tracing::warn!(
                follower_id = %follower_id,
                followee_id = %followee_id,
                count       = post_ids.len(),
                error       = %e,
                "Redis ZREM batch failed during unfollow prune"
            );
        }

        // ScyllaDB cleanup (background, best-effort).
        for (post_id, published_at_ms) in &authored_posts {
            if let Err(e) = self
                .feed_repository
                .delete(&follower_id, post_id, *published_at_ms)
                .await
            {
                tracing::warn!(
                    follower_id = %follower_id,
                    post_id     = %post_id,
                    error       = %e,
                    "ScyllaDB feed_items delete failed during unfollow prune"
                );
            }
        }

        tracing::debug!(
            follower_id = %follower_id,
            followee_id = %followee_id,
            pruned      = authored_posts.len(),
            "unfollow prune completed"
        );
        Ok(())
    }
}
