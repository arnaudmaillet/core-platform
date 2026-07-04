use async_trait::async_trait;
use chrono::Duration;
use fred::interfaces::KeysInterface;
use fred::types::Expiration;
use redis_storage::{RedisClient, RedisStorageError};
use tracing::instrument;

use crate::application::port::SessionCache;
use crate::domain::value_object::{AccountId, Generation, SessionId};
use crate::error::AuthError;

use super::keys::{blacklist_key, generation_key};

/// Redis Cluster implementation of [`SessionCache`].
#[derive(Clone)]
pub struct RedisSessionCache {
    client: RedisClient,
}

impl RedisSessionCache {
    pub fn new(client: RedisClient) -> Self {
        Self { client }
    }
}

fn cache_err(e: fred::error::Error) -> AuthError {
    AuthError::Cache(RedisStorageError::from(e))
}

#[async_trait]
impl SessionCache for RedisSessionCache {
    #[instrument(name = "auth.cache.current_generation", skip(self), fields(account.id = %account_id))]
    async fn current_generation(&self, account_id: &AccountId) -> Result<Generation, AuthError> {
        // A missing counter means "never bumped" ⇒ the initial generation.
        let value: Option<i64> =
            self.client.get(generation_key(account_id)).await.map_err(cache_err)?;
        Ok(value.map(Generation::from_i64).unwrap_or(Generation::INITIAL))
    }

    #[instrument(name = "auth.cache.bump_generation", skip(self), fields(account.id = %account_id))]
    async fn bump_generation(&self, account_id: &AccountId) -> Result<Generation, AuthError> {
        // INCR is atomic and creates the key at 1 on first use, which matches
        // `Generation::INITIAL.next()`.
        let next: i64 = self.client.incr(generation_key(account_id)).await.map_err(cache_err)?;
        Ok(Generation::from_i64(next))
    }

    #[instrument(name = "auth.cache.blacklist", skip(self), fields(session.id = %session_id))]
    async fn blacklist_session(
        &self,
        session_id: &SessionId,
        ttl: Duration,
    ) -> Result<(), AuthError> {
        let secs = ttl.num_seconds().max(1);
        let _: () = self
            .client
            .set(blacklist_key(session_id), "1", Some(Expiration::EX(secs)), None, false)
            .await
            .map_err(cache_err)?;
        Ok(())
    }

    #[instrument(name = "auth.cache.is_blacklisted", skip(self), fields(session.id = %session_id))]
    async fn is_blacklisted(&self, session_id: &SessionId) -> Result<bool, AuthError> {
        let count: i64 = self.client.exists(blacklist_key(session_id)).await.map_err(cache_err)?;
        Ok(count > 0)
    }
}
