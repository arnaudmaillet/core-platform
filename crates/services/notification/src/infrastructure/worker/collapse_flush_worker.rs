use std::sync::Arc;
use std::time::Duration;

use fred::interfaces::{LuaInterface, SortedSetsInterface};
use redis_storage::RedisClient;
use uuid::Uuid;

use crate::application::port::{NotificationRepository, StreamRegistry, UnreadCounter};
use crate::application::port::stream_registry::NotificationPayload;
use crate::config::NotificationConfig;
use crate::domain::aggregate::Notification;
use crate::domain::value_object::{
    NotificationId, NotificationKind, ProfileId, SubjectId, SubjectKind,
};
use crate::error::NotificationError;

// ── Lua scripts ───────────────────────────────────────────────────────────────

/// Atomically reads and deletes a collapse window.
///
/// KEYS[1] = notification:cw:{window_key}
/// KEYS[2] = notification:cw:{window_key}_:senders
/// Returns: [count_string, sender1, sender2, ...]
///          where count_string is "0" if the window is empty or already flushed.
const DRAIN_WINDOW_SCRIPT: &str = r#"
local window_key  = KEYS[1]
local senders_key = KEYS[2]

local count = redis.call('GET', window_key)
if not count or count == '0' then
    return {'0'}
end

local senders = redis.call('LRANGE', senders_key, 0, -1)
redis.call('DEL', window_key)
redis.call('DEL', senders_key)

local result = {count}
for _, s in ipairs(senders) do
    table.insert(result, s)
end
return result
"#;

// ── Worker ────────────────────────────────────────────────────────────────────

const SCHEDULE_KEY: &str = "notification:window_schedule";
const SCAN_BATCH:   usize = 100;

/// Periodic worker that settles cross-batch Redis collapse windows into ScyllaDB.
///
/// Every `flush_interval`, it queries the `notification:window_schedule` sorted
/// set for windows whose expiry score (Unix ms) has passed. For each settled
/// window it:
/// 1. Drains the Redis count + sender sample atomically via `DRAIN_WINDOW_SCRIPT`.
/// 2. Writes one collapsed `notifications_by_profile` row to ScyllaDB.
/// 3. Increments the unread counter.
/// 4. Broadcasts to any active gRPC stream subscriber.
/// 5. Removes the member from the schedule ZSET.
pub struct CollapseFlushWorker<R, U, S> {
    redis:          RedisClient,
    repository:     Arc<R>,
    counter:        Arc<U>,
    stream_reg:     Arc<S>,
    _config:        Arc<NotificationConfig>,
    flush_interval: Duration,
}

impl<R, U, S> CollapseFlushWorker<R, U, S>
where
    R: NotificationRepository,
    U: UnreadCounter,
    S: StreamRegistry,
{
    pub fn new(
        redis:          RedisClient,
        repository:     Arc<R>,
        counter:        Arc<U>,
        stream_reg:     Arc<S>,
        config:         Arc<NotificationConfig>,
        flush_interval: Duration,
    ) -> Self {
        Self { redis, repository, counter, stream_reg, _config: config, flush_interval }
    }

    pub async fn run(self) {
        let mut interval = tokio::time::interval(self.flush_interval);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            interval.tick().await;
            self.flush_cycle().await;
        }
    }

    async fn flush_cycle(&self) {
        let now_score = chrono::Utc::now().timestamp_millis() as f64;

        // Fetch all schedule members whose expiry score <= now.
        let members: Vec<String> = match self.redis.inner
            .zrangebyscore(
                SCHEDULE_KEY,
                "-inf",
                now_score.to_string().as_str(),
                false,
                Some((0i64, SCAN_BATCH as i64)),
            )
            .await
        {
            Ok(m) => m,
            Err(err) => {
                tracing::error!(error = %err, "CollapseFlushWorker: ZRANGEBYSCORE failed");
                return;
            }
        };

        if members.is_empty() {
            return;
        }

        tracing::debug!(count = members.len(), "collapse flush cycle: windows ready");

        for member in &members {
            if let Err(err) = self.flush_window(member).await {
                tracing::error!(
                    error  = %err,
                    window = member,
                    "collapse window flush failed"
                );
            }

            // Remove from schedule regardless of flush outcome — a failed flush
            // leaves no window data in Redis (DRAIN_WINDOW deletes it), so the
            // notification is silently dropped rather than double-written.
            let _: Result<i64, _> = self.redis.inner.zrem(SCHEDULE_KEY, member).await;
        }
    }

    async fn flush_window(&self, member: &str) -> Result<(), NotificationError> {
        // `member` format: `{target_uuid}:{subject_uuid}:{kind_str}`
        let parts: Vec<&str> = member.splitn(3, ':').collect();
        if parts.len() != 3 {
            tracing::warn!(member, "invalid schedule member format — skipping");
            return Ok(());
        }

        let target_uuid  = Uuid::parse_str(parts[0])
            .map_err(|_| NotificationError::InvalidProfileId(parts[0].to_owned()))?;
        let subject_uuid = Uuid::parse_str(parts[1])
            .map_err(|_| NotificationError::InvalidSubjectId(parts[1].to_owned()))?;
        let kind = match parts[2] {
            "reaction" => NotificationKind::Reaction,
            "comment"  => NotificationKind::Comment,
            "reply"    => NotificationKind::Reply,
            "mention"  => NotificationKind::Mention,
            other => {
                tracing::warn!(kind = other, "unknown kind in schedule member — skipping");
                return Ok(());
            }
        };

        let window_key  = format!("notification:cw:{}:{}:{}", target_uuid, subject_uuid, parts[2]);
        let senders_key = format!("{}_:senders", window_key);

        let result: Vec<String> = self.redis.inner
            .eval(
                DRAIN_WINDOW_SCRIPT,
                vec![window_key.clone(), senders_key],
                Vec::<String>::new(),
            )
            .await
            .map_err(|e| NotificationError::CollapseFlushFailed {
                window_key: window_key.clone(),
                message:    e.to_string(),
            })?;

        if result.is_empty() || result[0] == "0" {
            return Ok(());
        }

        let count: i32 = result[0].parse().unwrap_or(1);
        let sender_uuids: Vec<Uuid> = result[1..]
            .iter()
            .filter_map(|s| Uuid::parse_str(s).ok())
            .collect();

        if sender_uuids.is_empty() {
            return Ok(());
        }

        let target_id     = ProfileId::from_uuid(target_uuid);
        let subject_id    = SubjectId::from_uuid(subject_uuid);
        let primary_sender = ProfileId::from_uuid(sender_uuids[0]);
        let ntf_id        = NotificationId::new();
        let now           = chrono::Utc::now();

        let notification = Notification::create_collapsed(
            ntf_id,
            target_id,
            primary_sender,
            sender_uuids.clone(),
            count,
            kind,
            SubjectKind::Post,
            subject_id,
            now,
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
            target_profile_id = %target_id,
            subject_id        = %subject_id,
            sender_count      = count,
            kind              = kind.as_str(),
            "collapse window flushed to ScyllaDB"
        );

        Ok(())
    }
}
