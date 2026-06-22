use async_trait::async_trait;
use fred::interfaces::LuaInterface;
use redis_storage::RedisClient;

use crate::application::port::PresenceStore;
use crate::domain::value_object::{ConversationId, ProfileId};
use crate::error::ChatError;
use crate::infrastructure::cache::keys::{presence_key, typing_key};
use crate::infrastructure::cache::redis_err;
use crate::infrastructure::cache::script::{ZSET_ACTIVE, ZSET_HEARTBEAT, ZSET_REMOVE};

pub struct RedisPresenceStore {
    client: RedisClient,
}

impl RedisPresenceStore {
    pub fn new(client: RedisClient) -> Self {
        Self { client }
    }

    /// Heartbeats `member` into the expiring set at `key`.
    async fn heartbeat_into(
        &self,
        key:      String,
        member:   String,
        now_ms:   i64,
        ttl_secs: u64,
    ) -> Result<(), ChatError> {
        let min_ms = now_ms - (ttl_secs as i64 * 1_000);
        let _: i64 = self
            .client
            .inner
            .eval(
                ZSET_HEARTBEAT,
                vec![key],
                vec![now_ms.to_string(), member, min_ms.to_string(), ttl_secs.to_string()],
            )
            .await
            .map_err(redis_err)?;
        Ok(())
    }

    /// Lists members still alive at `key` (score >= now_ms - ttl).
    async fn active_in(
        &self,
        key:      String,
        now_ms:   i64,
        ttl_secs: u64,
    ) -> Result<Vec<ProfileId>, ChatError> {
        let min_ms = now_ms - (ttl_secs as i64 * 1_000);
        let raw: Vec<String> = self
            .client
            .inner
            .eval(ZSET_ACTIVE, vec![key], vec![min_ms.to_string()])
            .await
            .map_err(redis_err)?;
        raw.iter().map(|s| ProfileId::try_from(s.as_str())).collect()
    }
}

#[async_trait]
impl PresenceStore for RedisPresenceStore {
    async fn heartbeat(
        &self,
        conversation_id: &ConversationId,
        member_id:       &ProfileId,
        now_ms:          i64,
        ttl_secs:        u64,
    ) -> Result<(), ChatError> {
        self.heartbeat_into(presence_key(conversation_id), member_id.as_str(), now_ms, ttl_secs)
            .await
    }

    async fn online(
        &self,
        conversation_id: &ConversationId,
        now_ms:          i64,
        ttl_secs:        u64,
    ) -> Result<Vec<ProfileId>, ChatError> {
        self.active_in(presence_key(conversation_id), now_ms, ttl_secs).await
    }

    async fn leave(
        &self,
        conversation_id: &ConversationId,
        member_id:       &ProfileId,
    ) -> Result<(), ChatError> {
        let _: i64 = self
            .client
            .inner
            .eval(ZSET_REMOVE, vec![presence_key(conversation_id)], vec![member_id.as_str()])
            .await
            .map_err(redis_err)?;
        Ok(())
    }

    async fn start_typing(
        &self,
        conversation_id: &ConversationId,
        member_id:       &ProfileId,
        now_ms:          i64,
        ttl_secs:        u64,
    ) -> Result<(), ChatError> {
        self.heartbeat_into(typing_key(conversation_id), member_id.as_str(), now_ms, ttl_secs)
            .await
    }

    async fn typing(
        &self,
        conversation_id: &ConversationId,
        now_ms:          i64,
        ttl_secs:        u64,
    ) -> Result<Vec<ProfileId>, ChatError> {
        self.active_in(typing_key(conversation_id), now_ms, ttl_secs).await
    }
}
