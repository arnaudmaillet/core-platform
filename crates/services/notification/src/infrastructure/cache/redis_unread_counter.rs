use std::sync::Arc;

use async_trait::async_trait;
use fred::interfaces::{KeysInterface, LuaInterface};
use redis_storage::RedisClient;

use crate::application::port::{NotificationRepository, UnreadCounter};
use crate::config::NotificationConfig;
use crate::domain::value_object::ProfileId;
use crate::error::NotificationError;

// ── Lua scripts ───────────────────────────────────────────────────────────────

/// Atomically increments the unread counter, capped at `ARGV[1]`.
///
/// KEYS[1] = notification:unread:{profile_id}
/// ARGV[1] = cap (e.g. 99)
/// Returns: the new counter value (capped).
const INCR_CAPPED_SCRIPT: &str = r#"
local key = KEYS[1]
local cap = tonumber(ARGV[1])
local v   = redis.call('INCR', key)
if v > cap then
    redis.call('SET', key, tostring(cap))
    return cap
end
return v
"#;

/// Atomically decrements the unread counter, floored at 0.
///
/// KEYS[1] = notification:unread:{profile_id}
/// Returns: the new counter value (floored at 0).
const DECR_FLOORED_SCRIPT: &str = r#"
local key = KEYS[1]
local v   = tonumber(redis.call('GET', key) or '0')
if v <= 0 then
    redis.call('SET', key, '0')
    return 0
end
local nv = redis.call('DECR', key)
if nv < 0 then
    redis.call('SET', key, '0')
    return 0
end
return nv
"#;

// ── Key builders ──────────────────────────────────────────────────────────────

fn unread_key(profile_id: &ProfileId) -> String {
    format!("notification:unread:{}", profile_id)
}

fn horizon_key(profile_id: &ProfileId) -> String {
    format!("notification:read_horizon:{}", profile_id)
}

// ── Implementation ────────────────────────────────────────────────────────────

pub struct RedisUnreadCounter<R> {
    client:     RedisClient,
    repository: Arc<R>,
    config:     Arc<NotificationConfig>,
}

impl<R: NotificationRepository> RedisUnreadCounter<R> {
    pub fn new(client: RedisClient, repository: Arc<R>, config: Arc<NotificationConfig>) -> Self {
        Self { client, repository, config }
    }

    fn redis_err(e: fred::error::Error) -> NotificationError {
        NotificationError::Redis(redis_storage::RedisStorageError::from(e))
    }
}

#[async_trait]
impl<R: NotificationRepository> UnreadCounter for RedisUnreadCounter<R> {
    async fn increment(&self, profile_id: &ProfileId) -> Result<(), NotificationError> {
        let key = unread_key(profile_id);
        let cap = self.config.unread_cap;

        let _: i64 = self.client
            .inner
            .eval(
                INCR_CAPPED_SCRIPT,
                vec![key],
                vec![cap.to_string()],
            )
            .await
            .map_err(Self::redis_err)?;

        self.repository.increment_counter(profile_id).await?;
        Ok(())
    }

    async fn decrement(&self, profile_id: &ProfileId) -> Result<(), NotificationError> {
        let key = unread_key(profile_id);

        let _: i64 = self.client
            .inner
            .eval(
                DECR_FLOORED_SCRIPT,
                vec![key],
                Vec::<String>::new(),
            )
            .await
            .map_err(Self::redis_err)?;

        self.repository.decrement_counter(profile_id).await?;
        Ok(())
    }

    async fn reset(&self, profile_id: &ProfileId) -> Result<(), NotificationError> {
        let key = unread_key(profile_id);
        let _: () = self.client.inner
            .set(
                &key,
                "0",
                None,
                None,
                false,
            )
            .await
            .map_err(Self::redis_err)?;
        Ok(())
    }

    async fn get(&self, profile_id: &ProfileId) -> Result<i64, NotificationError> {
        let key = unread_key(profile_id);
        let raw: Option<String> = self.client.inner
            .get(&key)
            .await
            .map_err(Self::redis_err)?;

        if let Some(v) = raw {
            return Ok(v.parse::<i64>().unwrap_or(0));
        }

        // Cache miss: read from ScyllaDB counter and repopulate.
        let count = self.repository.read_counter(profile_id).await?;
        let capped = count.min(self.config.unread_cap);

        let _: () = self.client.inner
            .set(&key, capped.to_string(), None, None, false)
            .await
            .map_err(Self::redis_err)?;

        Ok(capped)
    }

    async fn set_read_horizon(
        &self,
        profile_id: &ProfileId,
        horizon_ms: i64,
    ) -> Result<(), NotificationError> {
        let key = horizon_key(profile_id);
        let _: () = self.client.inner
            .set(&key, horizon_ms.to_string(), None, None, false)
            .await
            .map_err(Self::redis_err)?;
        Ok(())
    }

    async fn get_read_horizon(&self, profile_id: &ProfileId) -> Result<i64, NotificationError> {
        let key = horizon_key(profile_id);
        let raw: Option<String> = self.client.inner
            .get(&key)
            .await
            .map_err(Self::redis_err)?;

        Ok(raw.and_then(|v| v.parse::<i64>().ok()).unwrap_or(0))
    }
}
