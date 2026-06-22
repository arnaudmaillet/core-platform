use std::sync::Arc;

use cqrs::{Command, CommandHandler, Envelope};
use validate_core::{FieldViolation, Validate};

use crate::application::port::{
    AuthorPostRepository, FeedRepository, FeedStore, FollowingStore, TierCache,
};
use crate::domain::value_object::{AuthorId, AuthorTier, FanOutMode, ProfileId};
use crate::error::TimelineError;

/// Triggered by `FollowCreatedWorker` when a `social-graph.followed` event arrives.
///
/// Backfill strategy:
///   Standard/Premium followee:
///     1. Query `posts_by_author` for the followee's most recent `backfill_limit` posts.
///     2. Batch ZADD into the follower's `timeline:feed:{follower_id}` ZSET.
///     3. Batch INSERT into `feed_items_by_profile` ScyllaDB for cold-start durability.
///   Vip followee:
///     NO-OP — the follower's read-path will automatically merge the VIP ZSET.
///
/// Additionally updates `timeline:following:{follower_id}` to include the new followee,
/// ensuring the read-path has an up-to-date following set for VIP merge routing.
pub struct BackfillFollowCommand {
    /// The profile that performed the follow action.
    pub follower_id: String,
    /// The profile that was followed.
    pub followee_id: String,
}

impl Command for BackfillFollowCommand {}

impl Validate for BackfillFollowCommand {
    fn validate(&self) -> Result<(), Vec<FieldViolation>> {
        let mut v = Vec::new();
        if self.follower_id.trim().is_empty() {
            v.push(FieldViolation::new("follower_id", "TML-VAL-020", "follower_id must not be empty"));
        }
        if self.followee_id.trim().is_empty() {
            v.push(FieldViolation::new("followee_id", "TML-VAL-021", "followee_id must not be empty"));
        }
        if v.is_empty() { Ok(()) } else { Err(v) }
    }
}

pub struct BackfillFollowHandler<FS, FR, AR, TC, FO> {
    pub feed_store:       Arc<FS>,
    pub feed_repository:  Arc<FR>,
    pub author_post_repo: Arc<AR>,
    pub tier_cache:       Arc<TC>,
    pub following_store:  Arc<FO>,
    pub feed_cap:         u16,
    pub backfill_limit:   i32,
}

impl<FS, FR, AR, TC, FO> CommandHandler<BackfillFollowCommand>
    for BackfillFollowHandler<FS, FR, AR, TC, FO>
where
    FS: FeedStore,
    FR: FeedRepository,
    AR: AuthorPostRepository,
    TC: TierCache,
    FO: FollowingStore,
{
    type Error = TimelineError;

    async fn handle(
        &self,
        envelope: Envelope<BackfillFollowCommand>,
    ) -> Result<(), TimelineError> {
        let cmd = &envelope.payload;

        let follower_id = ProfileId::try_from(cmd.follower_id.as_str())?;
        let followee_id = AuthorId::try_from(cmd.followee_id.as_str())?;

        // Track the new follow in the Redis following set for read-path routing.
        self.following_store.add(&follower_id, &followee_id).await?;

        // Resolve the followee's tier to decide whether to backfill.
        let tier = self
            .tier_cache
            .get_tier(&followee_id)
            .await?
            .unwrap_or(AuthorTier::Standard);

        if matches!(tier.fan_out_mode(), FanOutMode::Read) {
            // VIP authors: read-path automatically merges timeline:vip:{id}.
            tracing::debug!(
                follower_id = %follower_id,
                followee_id = %followee_id,
                "VIP follow — backfill skipped; following_store updated"
            );
            return Ok(());
        }

        // Standard/Premium: backfill the follower's feed with the followee's recent posts.
        let recent_posts = self
            .author_post_repo
            .list_recent(&followee_id, i64::MAX, self.backfill_limit)
            .await?;

        if recent_posts.is_empty() {
            return Ok(());
        }

        // Redis batch push (one Lua-capped ZADD per entry for correctness).
        if let Err(e) = self
            .feed_store
            .push_batch(&follower_id, &recent_posts, self.feed_cap)
            .await
        {
            tracing::warn!(
                follower_id = %follower_id,
                followee_id = %followee_id,
                count       = recent_posts.len(),
                error       = %e,
                "Redis backfill batch failed — ScyllaDB write-behind will provide cold-start recovery"
            );
        }

        // ScyllaDB write-behind for cold-start durability.
        if let Err(e) = self
            .feed_repository
            .insert_batch(&follower_id, &recent_posts)
            .await
        {
            tracing::warn!(
                follower_id = %follower_id,
                followee_id = %followee_id,
                count       = recent_posts.len(),
                error       = %e,
                "ScyllaDB backfill insert_batch failed"
            );
        }

        tracing::debug!(
            follower_id  = %follower_id,
            followee_id  = %followee_id,
            backfilled   = recent_posts.len(),
            "follow backfill completed"
        );
        Ok(())
    }
}
