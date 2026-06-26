use async_trait::async_trait;
use fred::interfaces::KeysInterface;
use fred::types::Expiration;
use redis_storage::{RedisClient, RedisStorageError};

use crate::application::port::DeliveryCache;
use crate::domain::aggregate::Asset;
use crate::domain::value_object::AssetId;
use crate::error::MediaError;

use super::keys::asset_key;

/// Time-to-live for a cached asset view. The cache is a read accelerator, not the
/// SoR — a miss simply rebuilds from Postgres.
const CACHE_TTL_SECS: i64 = 3600;

/// Redis implementation of [`DeliveryCache`]. Stores the asset as JSON; a
/// deserialize failure on read is treated as a miss (the read path fails open).
#[derive(Clone)]
pub struct RedisDeliveryCache {
    client: RedisClient,
}

impl RedisDeliveryCache {
    pub fn new(client: RedisClient) -> Self {
        Self { client }
    }
}

fn cache_err(e: fred::error::Error) -> MediaError {
    MediaError::Cache(RedisStorageError::from(e))
}

#[async_trait]
impl DeliveryCache for RedisDeliveryCache {
    async fn get(&self, id: &AssetId) -> Result<Option<Asset>, MediaError> {
        let raw: Option<String> = self.client.get(asset_key(id)).await.map_err(cache_err)?;
        // A corrupt/forward-incompatible cached value is a miss, not an error.
        Ok(raw.and_then(|s| serde_json::from_str::<Asset>(&s).ok()))
    }

    async fn put(&self, asset: &Asset) -> Result<(), MediaError> {
        let value = serde_json::to_string(asset).map_err(|e| MediaError::DomainViolation {
            field: "asset".into(),
            message: format!("failed to serialize asset for cache: {e}"),
        })?;
        let _: () = self
            .client
            .set(asset_key(&asset.id()), value, Some(Expiration::EX(CACHE_TTL_SECS)), None, false)
            .await
            .map_err(cache_err)?;
        Ok(())
    }

    async fn invalidate(&self, id: &AssetId) -> Result<(), MediaError> {
        let _: i64 = self.client.del(asset_key(id)).await.map_err(cache_err)?;
        Ok(())
    }
}
