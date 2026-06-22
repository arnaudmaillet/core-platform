use std::sync::Arc;

use cqrs::{Command, CommandHandler, Envelope};
use validate_core::{FieldViolation, Validate};

use crate::application::port::{
    AuthorPostRepository, FeedRepository, FeedStore, TierCache, VipRegistry,
};
use crate::domain::value_object::{AuthorId, AuthorTier, FanOutMode, PostId};
use crate::error::TimelineError;

/// Triggered by `PostDeletedWorker` when a `post.deleted` Kafka event arrives.
///
/// Deletion strategy by tier:
///   Vip: ZREM from `timeline:vip:{author_id}` + DELETE from `posts_by_author`.
///   Standard/Premium:
///     - DELETE from `posts_by_author`.
///     - Redis ZSET entries expire naturally or are pruned by the cap.
///       Eventual consistency is acceptable: BFF filters deleted posts during
///       hydration via the post service. Performing a full-follower ZREM storm
///       is explicitly avoided to prevent write-amplification on hot authors.
///     - ScyllaDB `feed_items_by_profile` entries are NOT deleted here because
///       partitioning by `profile_id` makes bulk delete by `post_id` impossible
///       without a secondary index. TTL (30 days) handles cleanup.
pub struct RemovePostCommand {
    pub post_id:  String,
    pub author_id: String,
    /// author_tier may be absent from older event schemas; default to Standard.
    pub author_tier: u8,
    pub published_at_ms: i64,
}

impl Command for RemovePostCommand {}

impl Validate for RemovePostCommand {
    fn validate(&self) -> Result<(), Vec<FieldViolation>> {
        let mut v = Vec::new();
        if self.post_id.trim().is_empty() {
            v.push(FieldViolation::new("post_id", "TML-VAL-010", "post_id must not be empty"));
        }
        if self.author_id.trim().is_empty() {
            v.push(FieldViolation::new("author_id", "TML-VAL-011", "author_id must not be empty"));
        }
        if v.is_empty() { Ok(()) } else { Err(v) }
    }
}

pub struct RemovePostHandler<FS, VR, FR, AR, TC> {
    pub feed_store:       Arc<FS>,
    pub vip_registry:     Arc<VR>,
    pub feed_repository:  Arc<FR>,
    pub author_post_repo: Arc<AR>,
    pub tier_cache:       Arc<TC>,
}

impl<FS, VR, FR, AR, TC> CommandHandler<RemovePostCommand>
    for RemovePostHandler<FS, VR, FR, AR, TC>
where
    FS: FeedStore,
    VR: VipRegistry,
    FR: FeedRepository,
    AR: AuthorPostRepository,
    TC: TierCache,
{
    type Error = TimelineError;

    async fn handle(
        &self,
        envelope: Envelope<RemovePostCommand>,
    ) -> Result<(), TimelineError> {
        let cmd = &envelope.payload;

        let post_id   = PostId::try_from(cmd.post_id.as_str())?;
        let author_id = AuthorId::try_from(cmd.author_id.as_str())?;

        // Resolve tier: prefer event payload, fall back to tier cache.
        let tier = if cmd.author_tier > 0 {
            AuthorTier::from_u8(cmd.author_tier)
        } else {
            self.tier_cache
                .get_tier(&author_id)
                .await?
                .unwrap_or(AuthorTier::Standard)
        };

        // Always remove from the per-author reverse index.
        if let Err(e) = self
            .author_post_repo
            .delete(&author_id, &post_id, cmd.published_at_ms)
            .await
        {
            tracing::warn!(
                post_id   = %post_id,
                author_id = %author_id,
                error     = %e,
                "posts_by_author delete failed"
            );
        }

        match tier.fan_out_mode() {
            FanOutMode::Read => {
                // VIP: remove from the Redis registry immediately.
                if let Err(e) = self.vip_registry.deregister(&author_id, &post_id).await {
                    tracing::warn!(
                        author_id = %author_id,
                        post_id   = %post_id,
                        error     = %e,
                        "VIP ZSET deregister failed"
                    );
                }
                tracing::debug!(
                    author_id = %author_id,
                    post_id   = %post_id,
                    "VIP post deregistered"
                );
            }
            FanOutMode::Write => {
                // Standard/Premium: eventual consistency via BFF filtering.
                // Redis ZSETs and ScyllaDB feed_items_by_profile are NOT purged
                // here to avoid write-amplification. The post TTL (30 days) and
                // BFF hydration filter handle cleanup transparently.
                tracing::debug!(
                    author_id = %author_id,
                    post_id   = %post_id,
                    "Standard/Premium post deleted from posts_by_author; Redis/feed_items deferred to TTL"
                );
            }
        }

        Ok(())
    }
}
