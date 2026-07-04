use async_trait::async_trait;
use fred::interfaces::KeysInterface;
use redis_storage::RedisClient;

use crate::application::port::TierCache;
use crate::domain::value_object::{AuthorId, AuthorTier, ProfileId};
use crate::error::TimelineError;

fn tier_key(author_id: &AuthorId) -> String {
    format!("timeline:tier:{}", author_id)
}

fn warm_key(profile_id: &ProfileId) -> String {
    format!("timeline:warm:{}", profile_id)
}

fn fred_err(e: fred::error::Error) -> TimelineError {
    TimelineError::Redis(redis_storage::RedisStorageError::from(e))
}

pub struct RedisTierCache {
    client: RedisClient,
}

impl RedisTierCache {
    pub fn new(client: RedisClient) -> Self {
        Self { client }
    }
}

#[async_trait]
impl TierCache for RedisTierCache {
    async fn get_tier(&self, author_id: &AuthorId) -> Result<Option<AuthorTier>, TimelineError> {
        let raw: Option<String> = self
            .client
            .inner
            .get(tier_key(author_id))
            .await
            .map_err(fred_err)?;

        Ok(raw.and_then(|s| s.parse::<u8>().ok()).map(AuthorTier::from_u8))
    }

    async fn get_tiers(
        &self,
        author_ids: &[AuthorId],
    ) -> Result<Vec<Option<AuthorTier>>, TimelineError> {
        if author_ids.is_empty() {
            return Ok(Vec::new());
        }

        // Per-author tier keys span cluster slots, so a single MGET would fail with
        // CROSSSLOT. Issue one GET per key concurrently instead — the client routes
        // each to the node owning its slot and pipelines them over the pool, turning
        // N sequential round-trips into one batched fan-out. Order is preserved.
        let gets = author_ids.iter().map(|author_id| {
            let key = tier_key(author_id);
            async move {
                let raw: Option<String> = self.client.inner.get(&key).await.map_err(fred_err)?;
                Ok::<Option<AuthorTier>, TimelineError>(
                    raw.and_then(|s| s.parse::<u8>().ok()).map(AuthorTier::from_u8),
                )
            }
        });

        futures::future::try_join_all(gets).await
    }

    async fn set_tier(
        &self,
        author_id: &AuthorId,
        tier:      AuthorTier,
        ttl_secs:  u64,
    ) -> Result<(), TimelineError> {
        self.client
            .inner
            .set::<(), _, _>(
                tier_key(author_id),
                tier.as_u8().to_string(),
                Some(fred::types::Expiration::EX(ttl_secs as i64)),
                None,
                false,
            )
            .await
            .map_err(fred_err)?;
        Ok(())
    }

    async fn is_warm(&self, profile_id: &ProfileId) -> Result<bool, TimelineError> {
        let exists: bool = self
            .client
            .inner
            .exists(warm_key(profile_id))
            .await
            .map_err(fred_err)?;
        Ok(exists)
    }

    async fn set_warm(&self, profile_id: &ProfileId, ttl_secs: u64) -> Result<(), TimelineError> {
        self.client
            .inner
            .set::<(), _, _>(
                warm_key(profile_id),
                "1",
                Some(fred::types::Expiration::EX(ttl_secs as i64)),
                None,
                false,
            )
            .await
            .map_err(fred_err)?;
        Ok(())
    }
}
