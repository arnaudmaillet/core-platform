use async_trait::async_trait;
use fred::interfaces::KeysInterface;
use fred::types::{Expiration, Value as FredValue};
use redis_storage::RedisClient;
use uuid::Uuid;

use crate::application::port::CardStore;
use crate::domain::entity::MapPostCard;
use crate::domain::value_object::{PostId, RetentionTtl};
use crate::error::GeoDiscoveryError;

// ── Key builder ───────────────────────────────────────────────────────────────

/// `sg:geo:card:{post_id}`
///
/// Cards are deliberately NOT hash-tagged: each post's card should distribute
/// across the cluster for even sharding. Bulk reads therefore fan out as
/// independent single-key GETs (see [`RedisCardStore::mget`]) rather than a
/// cross-slot `MGET`, which Redis Cluster rejects with CROSSSLOT.
fn card_key(post_id: &Uuid) -> String {
    format!("sg:geo:card:{}", post_id)
}

fn fred_err(e: fred::error::Error) -> GeoDiscoveryError {
    GeoDiscoveryError::Redis(redis_storage::RedisStorageError::from(e))
}

// ── RedisCardStore ────────────────────────────────────────────────────────────

pub struct RedisCardStore {
    client: RedisClient,
}

impl RedisCardStore {
    pub fn new(client: RedisClient) -> Self {
        Self { client }
    }
}

#[async_trait]
impl CardStore for RedisCardStore {
    async fn set(
        &self,
        card: &MapPostCard,
        ttl:  RetentionTtl,
    ) -> Result<(), GeoDiscoveryError> {
        let key   = card_key(&card.post_id);
        let bytes = rmp_serde::to_vec(card)
            .map_err(|e| GeoDiscoveryError::CardSerializationFailed {
                post_id: card.post_id.to_string(),
                message: e.to_string(),
            })?;

        let expiry = Expiration::EX(ttl.as_redis_ex() as i64);

        self.client.inner
            .set::<(), _, _>(&key, bytes.as_slice(), Some(expiry), None, false)
            .await
            .map_err(fred_err)?;

        Ok(())
    }

    async fn mget(
        &self,
        post_ids: &[Uuid],
    ) -> Result<Vec<Option<MapPostCard>>, GeoDiscoveryError> {
        if post_ids.is_empty() {
            return Ok(vec![]);
        }

        // Card keys span cluster slots, so a single MGET would fail with CROSSSLOT.
        // Issue one GET per key concurrently instead — the client routes each to the
        // node owning its slot, and the pool pipelines them. Order is preserved by
        // `try_join_all`.
        let gets = post_ids.iter().map(|id| {
            let key = card_key(id);
            async move {
                let value: FredValue = self.client.inner
                    .get(&key)
                    .await
                    .map_err(fred_err)?;

                match value {
                    FredValue::Bytes(bytes) => {
                        let card = rmp_serde::from_slice::<MapPostCard>(&bytes)
                            .map_err(|e| GeoDiscoveryError::CardDeserializationFailed {
                                post_id: id.to_string(),
                                message: e.to_string(),
                            })?;
                        Ok(Some(card))
                    }
                    FredValue::Null => Ok(None),
                    other => {
                        tracing::warn!(
                            post_id = %id,
                            kind    = ?other,
                            "unexpected Redis value type for card key — treating as cache miss"
                        );
                        Ok(None)
                    }
                }
            }
        });

        futures::future::try_join_all(gets).await
    }

    async fn del(
        &self,
        post_id: &PostId,
    ) -> Result<(), GeoDiscoveryError> {
        let key = card_key(&post_id.as_uuid());
        self.client.inner
            .del::<i64, _>(&key)
            .await
            .map_err(fred_err)?;
        Ok(())
    }
}
