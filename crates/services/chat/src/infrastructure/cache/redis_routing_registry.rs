use async_trait::async_trait;
use fred::interfaces::{KeysInterface, LuaInterface};
use redis_storage::RedisClient;

use crate::application::port::RoutingRegistry;
use crate::domain::value_object::ConversationId;
use crate::error::ChatError;
use crate::infrastructure::cache::keys::audience_shards_key;
use crate::infrastructure::cache::redis_err;
use crate::infrastructure::cache::script::{ZSET_ACTIVE, ZSET_HEARTBEAT, ZSET_REMOVE};

/// Audience-shard routing registry. Reuses the expiring-sorted-set pattern: an
/// active shard is a "member" kept alive by pod heartbeats, so a crashed pod's
/// shard ages out and the publisher stops fanning to it automatically.
pub struct RedisRoutingRegistry {
    client: RedisClient,
}

impl RedisRoutingRegistry {
    pub fn new(client: RedisClient) -> Self {
        Self { client }
    }
}

#[async_trait]
impl RoutingRegistry for RedisRoutingRegistry {
    async fn activate_shard(
        &self,
        conversation_id: &ConversationId,
        shard:           u16,
        now_ms:          i64,
        ttl_secs:        u64,
    ) -> Result<(), ChatError> {
        let min_ms = now_ms - (ttl_secs as i64 * 1_000);
        let _: i64 = self
            .client
            .inner
            .eval(
                ZSET_HEARTBEAT,
                vec![audience_shards_key(conversation_id)],
                vec![now_ms.to_string(), shard.to_string(), min_ms.to_string(), ttl_secs.to_string()],
            )
            .await
            .map_err(redis_err)?;
        Ok(())
    }

    async fn deactivate_shard(
        &self,
        conversation_id: &ConversationId,
        shard:           u16,
    ) -> Result<(), ChatError> {
        let _: i64 = self
            .client
            .inner
            .eval(
                ZSET_REMOVE,
                vec![audience_shards_key(conversation_id)],
                vec![shard.to_string()],
            )
            .await
            .map_err(redis_err)?;
        Ok(())
    }

    async fn active_shards(
        &self,
        conversation_id: &ConversationId,
        now_ms:          i64,
        ttl_secs:        u64,
    ) -> Result<Vec<u16>, ChatError> {
        let min_ms = now_ms - (ttl_secs as i64 * 1_000);
        let raw: Vec<String> = self
            .client
            .inner
            .eval(ZSET_ACTIVE, vec![audience_shards_key(conversation_id)], vec![min_ms.to_string()])
            .await
            .map_err(redis_err)?;

        raw.iter()
            .map(|s| {
                s.parse::<u16>().map_err(|_| ChatError::DomainViolation {
                    field:   "routing.active_shards".to_owned(),
                    message: format!("invalid shard index: '{s}'"),
                })
            })
            .collect()
    }

    async fn clear(&self, conversation_id: &ConversationId) -> Result<(), ChatError> {
        let _: i64 = self
            .client
            .inner
            .del(audience_shards_key(conversation_id))
            .await
            .map_err(redis_err)?;
        Ok(())
    }
}
