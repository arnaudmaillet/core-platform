use std::sync::Arc;

use fred::interfaces::KeysInterface;
use futures_util::StreamExt;
use redis_storage::RedisClient;
use serde::Deserialize;
use transport::kafka::consumer::builder::KafkaConsumerBuilder;
use transport::kafka::config::client::KafkaClientConfig;
use transport::kafka::config::consumer::{AutoOffsetReset, ConsumerConfig};

use crate::application::port::{BlockCache, NotificationRepository, StreamRegistry, UnreadCounter};
use crate::application::port::stream_registry::NotificationPayload;
use crate::config::NotificationConfig;
use crate::domain::aggregate::Notification;
use crate::domain::value_object::{
    NotificationId, NotificationKind, ProfileId, SubjectId, SubjectKind,
};
use crate::error::NotificationError;

const TOPIC_CREATED: &str = "comment.created";
const TOPIC_DELETED: &str = "comment.deleted";

// ── Minimal event projection ──────────────────────────────────────────────────

/// Projection of `comment.created` / `comment.deleted` Kafka events.
/// Unknown fields from the comment service schema are silently ignored.
#[derive(Debug, Deserialize)]
pub struct CommentEventPayload {
    pub comment_id:    String,
    pub post_id:       String,
    pub author_id:     String,
    /// `None` for top-level comments; `Some(comment_id)` for replies.
    pub parent_id:     Option<String>,
    pub created_at_ms: i64,
}

// ── Key builders ──────────────────────────────────────────────────────────────

fn post_author_key(post_id: &str) -> String {
    format!("notification:pa:{}", post_id)
}

fn comment_author_key(comment_id: &str) -> String {
    format!("notification:ca:{}", comment_id)
}

// ── Worker ────────────────────────────────────────────────────────────────────

/// Consumes `comment.created` events and produces:
/// - `COMMENT` notifications → the post author (when a top-level comment is added).
/// - `REPLY` notifications   → the parent comment's author.
///
/// Side effect: caches comment-author entries in Redis so future reply events
/// can look up the parent's author without a cross-service call.
pub struct CommentNotificationWorker<R, B, U, S> {
    kafka_config: KafkaClientConfig,
    redis:        RedisClient,
    repository:   Arc<R>,
    block_cache:  Arc<B>,
    counter:      Arc<U>,
    stream_reg:   Arc<S>,
    config:       Arc<NotificationConfig>,
    group_id:     String,
}

impl<R, B, U, S> CommentNotificationWorker<R, B, U, S>
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
                    tracing::warn!("comment notification consumer exited cleanly — restarting");
                }
                Err(e) => {
                    tracing::error!(
                        error = %e,
                        "comment notification consumer error — restarting after 5 s"
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
            .subscribe(TOPIC_CREATED)
            .subscribe(TOPIC_DELETED)
            .build()
            .map_err(|e| e.to_string())?;

        tracing::info!(
            topics = %format!("{}, {}", TOPIC_CREATED, TOPIC_DELETED),
            group  = %self.group_id,
            "comment notification consumer started"
        );

        let mut stream = handle.stream::<CommentEventPayload>();

        while let Some(result) = stream.next().await {
            let envelope = match result {
                Ok(e) => e,
                Err(err) => {
                    tracing::warn!(error = %err, "comment event deserialization error — skipping");
                    continue;
                }
            };

            // Only created events trigger notifications. Deletions do not.
            if envelope.topic.as_str() != TOPIC_CREATED {
                continue;
            }

            if let Err(err) = self.process(&envelope.payload).await {
                match err {
                    NotificationError::SenderBlocked { .. }
                    | NotificationError::SelfNotification { .. }
                    | NotificationError::PostAuthorCacheMiss { .. }
                    | NotificationError::CommentAuthorCacheMiss { .. } => {
                        tracing::debug!(
                            error      = %err,
                            comment_id = %envelope.payload.comment_id,
                            "comment notification suppressed"
                        );
                    }
                    other => {
                        tracing::error!(
                            error      = %other,
                            comment_id = %envelope.payload.comment_id,
                            "comment notification write failed"
                        );
                    }
                }
            }
        }

        Ok(())
    }

    async fn process(&self, event: &CommentEventPayload) -> Result<(), NotificationError> {
        let sender_id  = ProfileId::try_from(event.author_id.as_str())?;
        let subject_id = SubjectId::try_from(event.post_id.as_str())?;

        // Always cache this comment's author for future reply lookups.
        self.cache_comment_author(&event.comment_id, &sender_id).await;

        let (target_id, kind) = if let Some(ref parent_id_str) = event.parent_id {
            // REPLY: find the parent comment's author.
            let parent_author_key = comment_author_key(parent_id_str);
            let author_str: Option<String> = self.redis.inner.get(&parent_author_key).await
                .unwrap_or(None);

            let target_str = author_str.ok_or_else(|| NotificationError::CommentAuthorCacheMiss {
                comment_id: parent_id_str.clone(),
            })?;

            let target_id = ProfileId::try_from(target_str.as_str())?;
            (target_id, NotificationKind::Reply)
        } else {
            // COMMENT: find the post author.
            let post_author_key = post_author_key(&event.post_id);
            let author_str: Option<String> = self.redis.inner.get(&post_author_key).await
                .unwrap_or(None);

            let target_str = author_str.ok_or_else(|| NotificationError::PostAuthorCacheMiss {
                post_id: event.post_id.clone(),
            })?;

            let target_id = ProfileId::try_from(target_str.as_str())?;
            (target_id, NotificationKind::Comment)
        };

        // Self-notification guard.
        if sender_id == target_id {
            return Err(NotificationError::SelfNotification {
                profile_id: sender_id.as_str(),
            });
        }

        // Block gate.
        if self.block_cache.is_blocked(&sender_id, &target_id).await? {
            return Err(NotificationError::SenderBlocked {
                sender_id: sender_id.as_str(),
                target_id: target_id.as_str(),
            });
        }

        let ntf_id = NotificationId::new();
        let notification = Notification::create(
            ntf_id,
            target_id,
            sender_id,
            kind,
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
            comment_id        = %event.comment_id,
            target_profile_id = %target_id,
            kind              = kind.as_str(),
            "comment notification written"
        );

        Ok(())
    }

    async fn cache_comment_author(&self, comment_id: &str, author_id: &ProfileId) {
        let key = comment_author_key(comment_id);
        let ttl = self.config.comment_author_cache_ttl_secs;
        let _: Result<(), _> = self.redis.inner
            .set(
                &key,
                author_id.as_str(),
                Some(fred::types::Expiration::EX(ttl as i64)),
                None,
                false,
            )
            .await;
    }
}
