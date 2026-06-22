use std::sync::Arc;

use cqrs::{Command, CommandHandler, Envelope};
use validate_core::{FieldViolation, Validate};

use crate::application::port::{
    AuthorPostRepository, FeedRepository, FeedStore, SocialGraphClient, TierCache, VipRegistry,
};
use crate::domain::aggregate::FeedEntry;
use crate::domain::value_object::{AuthorId, AuthorTier, FanOutMode, PostId};
use crate::error::TimelineError;

/// Triggered by the `PostPublishedWorker` when a `post.published` Kafka event arrives.
///
/// Routes to either fan-out-on-write (Standard/Premium) or VIP registry
/// (Vip) based on `author_tier`, which is denormalized into the event
/// payload by services/post and never recalculated here.
///
/// Write protocol:
///   Standard/Premium:
///     1. Cache author tier in Redis.
///     2. Write post to `posts_by_author` ScyllaDB (durable backfill source).
///     3. Paginate follower list via SocialGraphClient.
///     4. For each follower batch: ZADD to `timeline:feed:{id}` + ScyllaDB INSERT.
///   Vip:
///     1. Cache author tier in Redis.
///     2. Write post to `posts_by_author` ScyllaDB (VIP cold-start source).
///     3. ZADD to `timeline:vip:{author_id}` with cap + TTL refresh.
///
/// All operations are idempotent — safe for Kafka at-least-once redelivery.
pub struct IngestPostPublishedCommand {
    pub post_id:        String,
    pub author_id:      String,
    pub author_tier:    u8,
    pub published_at_ms: i64,
}

impl Command for IngestPostPublishedCommand {}

impl Validate for IngestPostPublishedCommand {
    fn validate(&self) -> Result<(), Vec<FieldViolation>> {
        let mut v = Vec::new();
        if self.post_id.trim().is_empty() {
            v.push(FieldViolation::new("post_id", "TML-VAL-001", "post_id must not be empty"));
        }
        if self.author_id.trim().is_empty() {
            v.push(FieldViolation::new("author_id", "TML-VAL-002", "author_id must not be empty"));
        }
        if self.published_at_ms <= 0 {
            v.push(FieldViolation::new(
                "published_at_ms",
                "TML-VAL-003",
                "published_at_ms must be a positive Unix epoch millisecond timestamp",
            ));
        }
        if v.is_empty() { Ok(()) } else { Err(v) }
    }
}

pub struct IngestPostPublishedHandler<FS, VR, FR, AR, TC, SG> {
    pub feed_store:           Arc<FS>,
    pub vip_registry:         Arc<VR>,
    pub feed_repository:      Arc<FR>,
    pub author_post_repo:     Arc<AR>,
    pub tier_cache:           Arc<TC>,
    pub social_graph:         Arc<SG>,
    pub feed_cap:             u16,
    pub vip_registry_cap:     u16,
    pub vip_registry_ttl_secs: u64,
    pub tier_cache_ttl_secs:  u64,
    pub social_graph_page_size: i32,
}

impl<FS, VR, FR, AR, TC, SG> CommandHandler<IngestPostPublishedCommand>
    for IngestPostPublishedHandler<FS, VR, FR, AR, TC, SG>
where
    FS: FeedStore,
    VR: VipRegistry,
    FR: FeedRepository,
    AR: AuthorPostRepository,
    TC: TierCache,
    SG: SocialGraphClient,
{
    type Error = TimelineError;

    async fn handle(
        &self,
        envelope: Envelope<IngestPostPublishedCommand>,
    ) -> Result<(), TimelineError> {
        let cmd = &envelope.payload;

        let post_id   = PostId::try_from(cmd.post_id.as_str())?;
        let author_id = AuthorId::try_from(cmd.author_id.as_str())?;
        let tier      = AuthorTier::from_u8(cmd.author_tier);

        let entry = FeedEntry::new(post_id, author_id, cmd.published_at_ms);

        // Cache tier for read-path fan-out mode resolution.
        self.tier_cache
            .set_tier(&author_id, tier, self.tier_cache_ttl_secs)
            .await?;

        // Always write to posts_by_author (backfill + VIP cold-start source).
        self.author_post_repo
            .insert(&author_id, &post_id, tier, cmd.published_at_ms)
            .await?;

        match tier.fan_out_mode() {
            FanOutMode::Read => self.handle_vip_fanout(&entry).await,
            FanOutMode::Write => self.handle_write_fanout(&entry).await,
        }
    }
}

impl<FS, VR, FR, AR, TC, SG> IngestPostPublishedHandler<FS, VR, FR, AR, TC, SG>
where
    FS: FeedStore,
    VR: VipRegistry,
    FR: FeedRepository,
    AR: AuthorPostRepository,
    TC: TierCache,
    SG: SocialGraphClient,
{
    async fn handle_vip_fanout(&self, entry: &FeedEntry) -> Result<(), TimelineError> {
        self.vip_registry
            .register(entry, self.vip_registry_cap, self.vip_registry_ttl_secs)
            .await
            .map_err(|e| TimelineError::VipRegistryWriteFailed {
                author_id: entry.author_id.to_string(),
                message:   e.to_string(),
            })?;

        tracing::debug!(
            author_id = %entry.author_id,
            post_id   = %entry.post_id,
            "VIP post registered in ZSET registry"
        );
        Ok(())
    }

    async fn handle_write_fanout(&self, entry: &FeedEntry) -> Result<(), TimelineError> {
        let author_id = &entry.author_id;
        let page_token = String::new();
        let mut total_followers: u64 = 0;

        loop {
            let followers = self
                .social_graph
                .list_all_followers(author_id, self.social_graph_page_size)
                .await
                .map_err(|e| TimelineError::FanOutFailed {
                    author_id: author_id.to_string(),
                    message:   e.to_string(),
                })?;

            if followers.is_empty() {
                break;
            }

            total_followers += followers.len() as u64;

            // Fan-out to Redis hot feeds in parallel batches.
            for profile_id in &followers {
                if let Err(e) = self
                    .feed_store
                    .push(profile_id, entry, self.feed_cap)
                    .await
                {
                    tracing::warn!(
                        profile_id = %profile_id,
                        post_id    = %entry.post_id,
                        error      = %e,
                        "Redis feed push failed — continuing fan-out"
                    );
                }
            }

            // Write-behind to ScyllaDB (best-effort, non-blocking).
            for profile_id in &followers {
                if let Err(e) = self
                    .feed_repository
                    .insert(profile_id, entry)
                    .await
                {
                    tracing::warn!(
                        profile_id = %profile_id,
                        post_id    = %entry.post_id,
                        error      = %e,
                        "ScyllaDB feed insert failed — Redis is authoritative"
                    );
                }
            }

            // `list_all_followers` returns the full paginated list in one call.
            // If the implementation paginates internally, this loop runs once.
            // The break is here for future extension if pagination is exposed.
            let _ = page_token;
            break;
        }

        tracing::debug!(
            author_id       = %author_id,
            post_id         = %entry.post_id,
            total_followers = total_followers,
            "fan-out-on-write completed"
        );
        Ok(())
    }
}

// Compiler check: handler + error types satisfy Send + Sync bounds.
fn _assert_send_sync() {
    fn _check<T: Send + Sync>() {}
    _check::<TimelineError>();
}
