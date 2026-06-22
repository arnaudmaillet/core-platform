use std::sync::Arc;

use async_trait::async_trait;
use fred::interfaces::{KeysInterface, SetsInterface};
use redis_storage::RedisClient;

use crate::application::port::BlockCache;
use crate::config::NotificationConfig;
use crate::domain::value_object::ProfileId;
use crate::error::NotificationError;

/// Redis-backed block relationship cache.
///
/// Two-layer lookup:
///
/// 1. Point cache key `notification:block:{sender_id}:{target_id}` → "1" (TTL: block_cache_ttl_secs).
///    Populated by the social-graph service on block creation.
/// 2. Membership in the social-graph block set `social:blocks:{target_id}`.
///    This SMEMBERS set is maintained by the social-graph service and contains
///    all profile IDs that the target has blocked.
///
/// On cache miss: returns `false` (not blocked). The worst outcome is a single
/// spurious notification delivered immediately after a block is created — the
/// client UI filters this via the social-graph service on render.
pub struct RedisBlockCache {
    client: RedisClient,
    config: Arc<NotificationConfig>,
}

impl RedisBlockCache {
    pub fn new(client: RedisClient, config: Arc<NotificationConfig>) -> Self {
        Self { client, config }
    }

    fn point_key(sender_id: &ProfileId, target_id: &ProfileId) -> String {
        format!("notification:block:{}:{}", sender_id, target_id)
    }

    fn block_set_key(target_id: &ProfileId) -> String {
        format!("social:blocks:{}", target_id)
    }
}

#[async_trait]
impl BlockCache for RedisBlockCache {
    async fn is_blocked(
        &self,
        sender_id: &ProfileId,
        target_id: &ProfileId,
    ) -> Result<bool, NotificationError> {
        // Layer 1: point cache (fastest path, populated by social-graph service).
        let point_key = Self::point_key(sender_id, target_id);
        let hit: Option<String> = self.client.inner.get(&point_key).await.map_err(|e| {
            NotificationError::Redis(redis_storage::RedisStorageError::from(e))
        })?;

        if hit.as_deref() == Some("1") {
            return Ok(true);
        }

        // Layer 2: social-graph block set membership.
        let set_key    = Self::block_set_key(target_id);
        let member_val = sender_id.as_str();

        let is_member: bool = self.client.inner
            .sismember(&set_key, member_val.as_str())
            .await
            .map_err(|e| NotificationError::Redis(redis_storage::RedisStorageError::from(e)))?;

        if is_member {
            // Backfill the point cache for subsequent events.
            let ttl = self.config.block_cache_ttl_secs;
            let _: () = self.client.inner
                .set(
                    &point_key,
                    "1",
                    Some(fred::types::Expiration::EX(ttl as i64)),
                    None,
                    false,
                )
                .await
                .unwrap_or(());
        }

        Ok(is_member)
    }
}
