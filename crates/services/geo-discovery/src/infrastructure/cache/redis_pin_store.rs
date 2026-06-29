use async_trait::async_trait;
use fred::interfaces::KeysInterface;
use fred::types::{Expiration, Value as FredValue};
use redis_storage::RedisClient;
use uuid::Uuid;

use crate::application::port::PinStore;
use crate::domain::entity::RadarPin;
use crate::domain::value_object::{PostId, RetentionTtl};
use crate::error::GeoDiscoveryError;

// ── Key builder ───────────────────────────────────────────────────────────────

/// `sg:geo:pin:{post_id}`
///
/// As with cards, pins are deliberately NOT hash-tagged: each post's pin
/// distributes across the cluster for even sharding. Bulk reads therefore fan
/// out as independent single-key GETs rather than a cross-slot `MGET`, which
/// Redis Cluster rejects with CROSSSLOT.
fn pin_key(post_id: &Uuid) -> String {
    format!("sg:geo:pin:{}", post_id)
}

fn fred_err(e: fred::error::Error) -> GeoDiscoveryError {
    GeoDiscoveryError::Redis(redis_storage::RedisStorageError::from(e))
}

// ── RedisPinStore ─────────────────────────────────────────────────────────────

pub struct RedisPinStore {
    client: RedisClient,
}

impl RedisPinStore {
    pub fn new(client: RedisClient) -> Self {
        Self { client }
    }
}

#[async_trait]
impl PinStore for RedisPinStore {
    async fn set(
        &self,
        pin: &RadarPin,
        ttl: RetentionTtl,
    ) -> Result<(), GeoDiscoveryError> {
        let key   = pin_key(&pin.post_id);
        let bytes = rmp_serde::to_vec(pin)
            .map_err(|e| GeoDiscoveryError::CardSerializationFailed {
                post_id: pin.post_id.to_string(),
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
    ) -> Result<Vec<Option<RadarPin>>, GeoDiscoveryError> {
        if post_ids.is_empty() {
            return Ok(vec![]);
        }

        // Pin keys span cluster slots, so a single MGET would fail with CROSSSLOT.
        // Issue one GET per key concurrently; the client routes each to the node
        // owning its slot and the pool pipelines them. Order is preserved by
        // `try_join_all`.
        let gets = post_ids.iter().map(|id| {
            let key = pin_key(id);
            async move {
                let value: FredValue = self.client.inner
                    .get(&key)
                    .await
                    .map_err(fred_err)?;

                match value {
                    FredValue::Bytes(bytes) => {
                        let pin = rmp_serde::from_slice::<RadarPin>(&bytes)
                            .map_err(|e| GeoDiscoveryError::CardDeserializationFailed {
                                post_id: id.to_string(),
                                message: e.to_string(),
                            })?;
                        Ok(Some(pin))
                    }
                    FredValue::Null => Ok(None),
                    other => {
                        tracing::warn!(
                            post_id = %id,
                            kind    = ?other,
                            "unexpected Redis value type for pin key — treating as miss"
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
        let key = pin_key(&post_id.as_uuid());
        self.client.inner
            .del::<i64, _>(&key)
            .await
            .map_err(fred_err)?;
        Ok(())
    }
}
