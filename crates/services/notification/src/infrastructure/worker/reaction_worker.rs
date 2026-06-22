use std::collections::HashMap;
use std::sync::Arc;

use fred::interfaces::{KeysInterface, LuaInterface};
use futures_util::StreamExt;
use redis_storage::RedisClient;
use serde::{Deserialize, Serialize};
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
use crate::infrastructure::worker::collapse::{CollapseBuffer, CollapseKey};

const TOPIC: &str = "engagement.reactions";

// ── Lua scripts ───────────────────────────────────────────────────────────────

/// Atomically increments the subject heat counter with a rolling 5-minute TTL.
///
/// KEYS[1] = notification:hot:{subject_id}
/// ARGV[1] = hot_ttl_secs (e.g. 300)
/// Returns: the new counter value.
const INCR_HOT_SCRIPT: &str = r#"
local key = KEYS[1]
local ttl = tonumber(ARGV[1])
local v   = redis.call('INCR', key)
if v == 1 then
    redis.call('EXPIRE', key, ttl)
end
return v
"#;

/// Accumulates a sender into the cross-batch collapse window.
///
/// KEYS[1] = notification:cw:{target}:{subject}:{kind}       (INCR counter)
/// KEYS[2] = notification:cw:{target}:{subject}:{kind}_:senders  (RPUSH sample list)
/// ARGV[1] = sender_id (string)
/// ARGV[2] = window_ttl_secs
/// ARGV[3] = max_sample_senders
/// ARGV[4] = schedule_member (for the ZSET)
/// ARGV[5] = window_expiry_score (Unix ms when the window should be flushed)
/// Returns: total count in window.
const COLLAPSE_ACCUMULATE_SCRIPT: &str = r#"
local window_key    = KEYS[1]
local senders_key   = KEYS[2]
local sender_id     = ARGV[1]
local ttl           = tonumber(ARGV[2])
local max_sample    = tonumber(ARGV[3])
local sched_member  = ARGV[4]
local expiry_score  = tonumber(ARGV[5])

local count = redis.call('INCR', window_key)
if count == 1 then
    redis.call('EXPIRE', window_key,  ttl)
    redis.call('EXPIRE', senders_key, ttl + 10)
    redis.call('ZADD', 'notification:window_schedule', expiry_score, sched_member)
end

local slen = redis.call('LLEN', senders_key)
if slen < max_sample then
    redis.call('RPUSH', senders_key, sender_id)
end

return count
"#;

/// Checks the hourly notification cap and increments it atomically.
///
/// KEYS[1] = notification:cap:{profile}:{subject}:{kind}:{yyyymmddhh}
/// ARGV[1] = max_per_hour
/// Returns: 1 if the notification is allowed, 0 if capped.
const HOURLY_CAP_SCRIPT: &str = r#"
local key = KEYS[1]
local max = tonumber(ARGV[1])
local v   = redis.call('INCR', key)
if v == 1 then
    redis.call('EXPIRE', key, 3600)
end
if v > max then
    return 0
end
return 1
"#;

// ── Payload shape (from engagement.reactions topic) ───────────────────────────

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "event_type", rename_all = "snake_case")]
enum ReactionKafkaEvent {
    Upserted(ReactionUpsertedPayload),
    Removed(ReactionRemovedPayload),
}

#[derive(Debug, Deserialize, Serialize)]
struct ReactionUpsertedPayload {
    pub post_id:     String,
    pub profile_id:  String,
    pub event_at_ms: i64,
}

#[derive(Debug, Deserialize, Serialize)]
struct ReactionRemovedPayload {
    pub post_id:     String,
    pub profile_id:  String,
    pub event_at_ms: i64,
}

// ── Key builders ──────────────────────────────────────────────────────────────

fn hot_key(subject_id: &SubjectId) -> String {
    format!("notification:hot:{}", subject_id)
}

fn post_author_key(post_id: &str) -> String {
    format!("notification:pa:{}", post_id)
}

fn cap_key(target_id: &ProfileId, subject_id: &SubjectId, kind: &str) -> String {
    let hour = chrono::Utc::now().format("%Y%m%d%H");
    format!("notification:cap:{}:{}:{}:{}", target_id, subject_id, kind, hour)
}

// ── Worker ────────────────────────────────────────────────────────────────────

pub struct ReactionNotificationWorker<R, B, U, S> {
    kafka_config: KafkaClientConfig,
    redis:        RedisClient,
    repository:   Arc<R>,
    block_cache:  Arc<B>,
    counter:      Arc<U>,
    stream_reg:   Arc<S>,
    config:       Arc<NotificationConfig>,
    group_id:     String,
}

impl<R, B, U, S> ReactionNotificationWorker<R, B, U, S>
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
                    tracing::warn!("reaction notification consumer exited cleanly — restarting");
                }
                Err(e) => {
                    tracing::error!(
                        error = %e,
                        "reaction notification consumer error — restarting after 5 s"
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

        tracing::info!(topic = TOPIC, group = %self.group_id, "reaction notification consumer started");

        let mut stream = handle.stream::<ReactionKafkaEvent>();

        // ── In-batch collapse accumulator ─────────────────────────────────────
        // Populated during one poll cycle, flushed at the end of each batch.
        let mut batch: HashMap<CollapseKey, CollapseBuffer> = HashMap::new();

        while let Some(result) = stream.next().await {
            let envelope = match result {
                Ok(e) => e,
                Err(err) => {
                    tracing::warn!(error = %err, "reaction event deserialization error — skipping");
                    continue;
                }
            };

            let (post_id, sender_str, event_at_ms) = match &envelope.payload {
                ReactionKafkaEvent::Upserted(e) =>
                    (e.post_id.as_str(), e.profile_id.as_str(), e.event_at_ms),
                ReactionKafkaEvent::Removed(_) => {
                    // Reaction removals do not generate notifications.
                    continue;
                }
            };

            let sender_uuid = match Uuid::parse_str(sender_str) {
                Ok(u) => u,
                Err(_) => {
                    tracing::warn!(profile_id = sender_str, "invalid sender UUID — skipping");
                    continue;
                }
            };

            // Look up post author from Redis cache (populated by MentionWorker).
            let author_key = post_author_key(post_id);
            let author_str: Option<String> = match self.redis.inner.get(&author_key).await {
                Ok(v)    => v,
                Err(err) => {
                    tracing::warn!(error = %err, post_id, "Redis get post_author failed");
                    None
                }
            };

            let target_str = match author_str {
                Some(s) => s,
                None => {
                    tracing::debug!(
                        post_id,
                        "post author cache miss — reaction notification suppressed (post not yet indexed)"
                    );
                    continue;
                }
            };

            let target_uuid = match Uuid::parse_str(&target_str) {
                Ok(u) => u,
                Err(_) => {
                    tracing::warn!(post_id, target = target_str, "invalid target UUID — skipping");
                    continue;
                }
            };

            // Self-notification guard.
            if sender_uuid == target_uuid {
                continue;
            }

            let subject_uuid = match Uuid::parse_str(post_id) {
                Ok(u) => u,
                Err(_) => {
                    tracing::warn!(post_id, "invalid post UUID — skipping");
                    continue;
                }
            };

            let key = CollapseKey::new(
                target_uuid,
                subject_uuid,
                SubjectKind::Post,
                NotificationKind::Reaction,
            );

            batch
                .entry(key)
                .and_modify(|buf| {
                    buf.push(sender_uuid, event_at_ms, self.config.max_sample_senders)
                })
                .or_insert_with(|| {
                    CollapseBuffer::new(
                        sender_uuid,
                        event_at_ms,
                        self.config.max_sample_senders,
                    )
                });

            // Flush the batch after accumulating (stream yields one message at a time
            // in this loop; the batch is flushed here to respect the collapse window).
            // In production, replace with a fixed-size batch poll for higher throughput.
            if batch.len() >= 500 {
                self.flush_batch(&mut batch).await;
            }
        }

        // Flush any remaining accumulated events.
        if !batch.is_empty() {
            self.flush_batch(&mut batch).await;
        }

        Ok(())
    }

    async fn flush_batch(&self, batch: &mut HashMap<CollapseKey, CollapseBuffer>) {
        let drained: Vec<(CollapseKey, CollapseBuffer)> = batch.drain().collect();

        for (key, buf) in drained {
            if let Err(err) = self.process_collapsed(&key, &buf).await {
                match err {
                    NotificationError::SenderBlocked { .. }
                    | NotificationError::SelfNotification { .. } => {
                        tracing::debug!(error = %err, "notification suppressed by gate");
                    }
                    other => {
                        tracing::error!(
                            error             = %other,
                            target_profile_id = %key.target_profile_id,
                            subject_id        = %key.subject_id,
                            "reaction notification write failed"
                        );
                    }
                }
            }
        }
    }

    async fn process_collapsed(
        &self,
        key: &CollapseKey,
        buf: &CollapseBuffer,
    ) -> Result<(), NotificationError> {
        let target_id = ProfileId::from_uuid(key.target_profile_id);
        let sender_id = ProfileId::from_uuid(buf.primary_sender());
        let subject_id = SubjectId::from_uuid(key.subject_id);

        // Block gate.
        if self.block_cache.is_blocked(&sender_id, &target_id).await? {
            return Err(NotificationError::SenderBlocked {
                sender_id: sender_id.as_str(),
                target_id: target_id.as_str(),
            });
        }

        // Subject heat detection — activate cross-batch window for hot subjects.
        let hot_key_str  = hot_key(&subject_id);
        let heat: i64 = self.redis.inner
            .eval(
                INCR_HOT_SCRIPT,
                vec![hot_key_str],
                vec![300u64.to_string()],
            )
            .await
            .unwrap_or(0i64);

        let is_hot = heat >= self.config.hot_subject_threshold as i64;

        if is_hot && buf.total_count == 1 {
            // For hot subjects with a single sender in this batch, defer to the
            // cross-batch collapse window instead of writing immediately.
            return self.accumulate_in_window(key, buf, &sender_id).await;
        }

        // Hourly hard cap check.
        let cap_key_str = cap_key(&target_id, &subject_id, key.kind.as_str());
        let allowed: i64 = self.redis.inner
            .eval(
                HOURLY_CAP_SCRIPT,
                vec![cap_key_str],
                vec![self.config.max_notifications_per_subject_per_hour.to_string()],
            )
            .await
            .unwrap_or(1i64);

        if allowed == 0 {
            tracing::debug!(
                target_profile_id = %target_id,
                subject_id        = %subject_id,
                "notification capped by hourly rate limit"
            );
            return Ok(());
        }

        // Write collapsed notification to ScyllaDB.
        let ntf_id = NotificationId::new();
        let notification = Notification::create_collapsed(
            ntf_id,
            target_id,
            sender_id,
            buf.sample_sender_ids(),
            buf.sender_count(),
            key.kind,
            key.subject_kind,
            subject_id,
            chrono::Utc::now(),
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

        Ok(())
    }

    async fn accumulate_in_window(
        &self,
        key:       &CollapseKey,
        _buf:      &CollapseBuffer,
        sender_id: &ProfileId,
    ) -> Result<(), NotificationError> {
        let window_key   = key.redis_window_key();
        let senders_key  = key.redis_senders_key();
        let ttl          = self.config.collapse_window_secs;
        let max_sample   = self.config.max_sample_senders;
        let sched_member = key.schedule_member();
        let expiry_score = (chrono::Utc::now().timestamp_millis()
            + (ttl as i64 * 1000)) as f64;

        let _: i64 = self.redis.inner
            .eval(
                COLLAPSE_ACCUMULATE_SCRIPT,
                vec![window_key, senders_key],
                vec![
                    sender_id.as_str(),
                    ttl.to_string(),
                    max_sample.to_string(),
                    sched_member,
                    (expiry_score as i64).to_string(),
                ],
            )
            .await
            .map_err(|e| NotificationError::Redis(redis_storage::RedisStorageError::from(e)))?;

        Ok(())
    }
}
