use async_trait::async_trait;
use chrono::{DateTime, Utc};
use fred::interfaces::KeysInterface;
use fred::types::Expiration;
use redis_storage::{RedisClient, RedisStorageError};

use crate::application::port::EnforcementProjection;
use crate::domain::value_object::{ActorId, EnforcementVersion};
use crate::error::ModerationError;

use super::keys::enforcement_key;

/// Redis Cluster implementation of [`EnforcementProjection`] — the Plane B
/// hot-path flag the fleet reads to answer "is this actor restricted right now".
///
/// The key value is the [`EnforcementVersion`] under which the restriction was
/// written; a present key means restricted. Writes are version-guarded best-effort
/// (read-then-act): a lower-versioned write is ignored so a stale reversal cannot
/// clear a newer restriction. Per-actor event ordering (Kafka keys by `actor_id`)
/// makes a true race vanishingly unlikely.
#[derive(Clone)]
pub struct RedisEnforcementProjection {
    client: RedisClient,
}

impl RedisEnforcementProjection {
    pub fn new(client: RedisClient) -> Self {
        Self { client }
    }
}

fn cache_err(e: fred::error::Error) -> ModerationError {
    ModerationError::Cache(RedisStorageError::from(e))
}

#[async_trait]
impl EnforcementProjection for RedisEnforcementProjection {
    async fn set_actor_restriction(
        &self,
        actor_id: &ActorId,
        version: EnforcementVersion,
        expires_at: Option<DateTime<Utc>>,
    ) -> Result<(), ModerationError> {
        let key = enforcement_key(actor_id);
        let current: Option<i64> = self.client.get(&key).await.map_err(cache_err)?;
        if current.is_some_and(|c| version.value() < c) {
            return Ok(()); // stale write — a newer restriction already stands.
        }
        let expiration = expires_at.map(|exp| {
            let secs = (exp - Utc::now()).num_seconds().max(1);
            Expiration::EX(secs)
        });
        let _: () = self
            .client
            .set(&key, version.value(), expiration, None, false)
            .await
            .map_err(cache_err)?;
        Ok(())
    }

    async fn clear_actor_restriction(
        &self,
        actor_id: &ActorId,
        version: EnforcementVersion,
    ) -> Result<(), ModerationError> {
        let key = enforcement_key(actor_id);
        let current: Option<i64> = self.client.get(&key).await.map_err(cache_err)?;
        // Only clear when our version is at least the stored one.
        if current.is_none_or(|c| version.value() >= c) {
            let _: i64 = self.client.del(&key).await.map_err(cache_err)?;
        }
        Ok(())
    }

    async fn is_actor_restricted(&self, actor_id: &ActorId) -> Result<bool, ModerationError> {
        let count: i64 = self.client.exists(enforcement_key(actor_id)).await.map_err(cache_err)?;
        Ok(count > 0)
    }
}
