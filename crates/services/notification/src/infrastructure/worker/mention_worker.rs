use std::sync::Arc;

use fred::interfaces::KeysInterface;
use futures_util::StreamExt;
use once_cell::sync::Lazy;
use redis_storage::RedisClient;
use regex::Regex;
use serde::Deserialize;
use transport::kafka::consumer::builder::KafkaConsumerBuilder;
use transport::kafka::config::client::KafkaClientConfig;
use transport::kafka::config::consumer::{AutoOffsetReset, ConsumerConfig};
use uuid::Uuid;

use crate::application::port::{BlockCache, NotificationRepository, StreamRegistry, UnreadCounter};
use crate::application::port::stream_registry::NotificationPayload;
use crate::config::NotificationConfig;
use crate::domain::aggregate::Notification;
use crate::domain::value_object::{
    NotificationId, NotificationKind, ProfileId, SubjectId, SubjectKind,
};
use crate::error::NotificationError;

const TOPIC: &str = "post.published";

// ── Mention regex ─────────────────────────────────────────────────────────────

/// Matches `[@handle](profile:UUID)` mention tokens in post captions.
/// Capture group 1 is the profile UUID string.
static MENTION_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\[@[^\]]*\]\(profile:([0-9a-fA-F-]{36})\)")
        .expect("mention regex is valid")
});

// ── Payload shape ─────────────────────────────────────────────────────────────

/// Minimal projection of `post.published` Kafka events.
/// Unknown fields from the post service schema are silently ignored.
#[derive(Debug, Deserialize)]
pub struct PostPublishedPayload {
    pub post_id:   String,
    pub author_id: String,
    /// The raw post caption, used for mention token extraction.
    pub caption:   Option<String>,
}

// ── Key builders ──────────────────────────────────────────────────────────────

fn post_author_key(post_id: &str) -> String {
    format!("notification:pa:{}", post_id)
}

// ── Worker ────────────────────────────────────────────────────────────────────

/// Consumes `post.published` events and:
///
/// 1. Caches `notification:pa:{post_id}` → `author_profile_id` in Redis so the
///    `ReactionNotificationWorker` can resolve the notification target without a
///    cross-service call.
/// 2. Parses `[@handle](profile:UUID)` mention tokens from the post caption and
///    writes one `MENTION` notification per unique mentioned profile.
pub struct MentionNotificationWorker<R, B, U, S> {
    kafka_config: KafkaClientConfig,
    redis:        RedisClient,
    repository:   Arc<R>,
    block_cache:  Arc<B>,
    counter:      Arc<U>,
    stream_reg:   Arc<S>,
    config:       Arc<NotificationConfig>,
    group_id:     String,
}

impl<R, B, U, S> MentionNotificationWorker<R, B, U, S>
where
    R: NotificationRepository,
    B: BlockCache,
    U: UnreadCounter,
    S: StreamRegistry,
{
    pub fn new(
        kafka_config: KafkaClientConfig,
        redis:        RedisClient,
        repository:   Arc<R>,
        block_cache:  Arc<B>,
        counter:      Arc<U>,
        stream_reg:   Arc<S>,
        config:       Arc<NotificationConfig>,
        group_id:     impl Into<String>,
    ) -> Self {
        Self {
            kafka_config,
            redis,
            repository,
            block_cache,
            counter,
            stream_reg,
            config,
            group_id: group_id.into(),
        }
    }

    pub async fn run(self) {
        loop {
            match self.run_once().await {
                Ok(()) => {
                    tracing::warn!("mention notification consumer exited cleanly — restarting");
                }
                Err(e) => {
                    tracing::error!(
                        error = %e,
                        "mention notification consumer error — restarting after 5 s"
                    );
                    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                }
            }
        }
    }

    async fn run_once(&self) -> Result<(), String> {
        let mut config = ConsumerConfig::new(self.kafka_config.clone(), &self.group_id);
        config.auto_offset_reset  = AutoOffsetReset::Earliest;
        config.enable_auto_commit = true;

        let handle = KafkaConsumerBuilder::new(config)
            .subscribe(TOPIC)
            .build()
            .map_err(|e| e.to_string())?;

        tracing::info!(topic = TOPIC, group = %self.group_id, "mention notification consumer started");

        let mut stream = handle.stream::<PostPublishedPayload>();

        while let Some(result) = stream.next().await {
            let envelope = match result {
                Ok(e) => e,
                Err(err) => {
                    tracing::warn!(error = %err, "post.published deserialization error — skipping");
                    continue;
                }
            };

            if let Err(err) = self.process(&envelope.payload).await {
                tracing::error!(
                    error   = %err,
                    post_id = %envelope.payload.post_id,
                    "mention worker failed to process post"
                );
            }
        }

        Ok(())
    }

    async fn process(&self, event: &PostPublishedPayload) -> Result<(), NotificationError> {
        // Step 1: cache post author for the reaction worker.
        self.cache_post_author(&event.post_id, &event.author_id).await;

        // Step 2: extract mention UUIDs from caption.
        let caption = match event.caption.as_deref().filter(|c| !c.is_empty()) {
            Some(c) => c,
            None    => return Ok(()),
        };

        let sender_id = ProfileId::try_from(event.author_id.as_str())?;
        let subject_id = SubjectId::try_from(event.post_id.as_str())?;

        let mut seen: Vec<Uuid> = Vec::new();

        for cap in MENTION_REGEX.captures_iter(caption) {
            let uuid_str = &cap[1];
            let mentioned_uuid = match Uuid::parse_str(uuid_str) {
                Ok(u)  => u,
                Err(_) => {
                    tracing::warn!(uuid = uuid_str, "invalid UUID in mention token — skipping");
                    continue;
                }
            };

            // Deduplicate mentions of the same profile within one post.
            if seen.contains(&mentioned_uuid) {
                continue;
            }
            seen.push(mentioned_uuid);

            let target_id = ProfileId::from_uuid(mentioned_uuid);

            // Self-mention guard.
            if sender_id == target_id {
                continue;
            }

            // Block gate.
            match self.block_cache.is_blocked(&sender_id, &target_id).await {
                Ok(true) => {
                    tracing::debug!(
                        sender_id = %sender_id,
                        target_id = %target_id,
                        "mention notification suppressed by block"
                    );
                    continue;
                }
                Ok(false) => {}
                Err(err) => {
                    tracing::warn!(error = %err, "block cache error — proceeding without block check");
                }
            }

            let ntf_id = NotificationId::new();
            let notification = Notification::create(
                ntf_id,
                target_id,
                sender_id,
                NotificationKind::Mention,
                SubjectKind::Post,
                subject_id,
            );

            self.repository.insert(&notification).await?;
            self.counter.increment(&target_id).await?;

            let payload = Arc::new(NotificationPayload {
                notification_id:   notification.id().as_uuid(),
                target_profile_id: notification.target_profile_id().as_uuid(),
                sender_profile_id: notification.sender_profile_id().as_uuid(),
                sample_sender_ids: notification.sample_sender_ids().to_vec(),
                sender_count:      notification.sender_count(),
                kind:              notification.kind(),
                subject_kind:      notification.subject_kind(),
                subject_id:        notification.subject_id().as_uuid(),
                created_at_ms:     notification.created_at().timestamp_millis(),
            });
            self.stream_reg.broadcast(&target_id, payload);

            tracing::debug!(
                post_id   = %event.post_id,
                mentioned = %target_id,
                "mention notification written"
            );
        }

        Ok(())
    }

    async fn cache_post_author(&self, post_id: &str, author_id: &str) {
        let key = post_author_key(post_id);
        let ttl = self.config.post_author_cache_ttl_secs;
        let _: Result<(), _> = self.redis.inner
            .set(
                &key,
                author_id,
                Some(fred::types::Expiration::EX(ttl as i64)),
                None,
                false,
            )
            .await;
    }
}
