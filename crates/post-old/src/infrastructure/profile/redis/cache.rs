// crates/post/src/infrastructure/profile/redis/cache.rs

use crate::application::cache::ProfileCacheRepository;
use crate::infrastructure::profile::redis::RedisProfileModel;
use async_trait::async_trait;
use infra_fred::fred;
use infra_fred::fred::interfaces::KeysInterface;
use infra_fred::fred::types::Expiration;
use shared_kernel::core::{Error, Result};
use shared_kernel::types::ProfileId;
use shared_proto::profile::v1::ProfileSummaryDto;
use std::time::Duration;

pub struct RedisProfileCache {
    redis_pool: fred::clients::Pool,
}

impl RedisProfileCache {
    pub fn new(redis_pool: fred::clients::Pool) -> Self {
        Self { redis_pool }
    }

    fn build_key(&self, profile_id: &ProfileId) -> String {
        format!("profiles:compact:{}", profile_id)
    }
}

#[async_trait]
impl ProfileCacheRepository for RedisProfileCache {
    async fn get(&self, profile_id: &ProfileId) -> Result<Option<ProfileSummaryDto>> {
        let key = self.build_key(profile_id);

        let cached_bytes: Option<String> = self
            .redis_pool
            .get(&key)
            .await
            .map_err(|e| Error::database(format!("Redis GET profile failed: {}", e)))?;

        match cached_bytes {
            Some(json_str) => {
                let dto = RedisProfileModel::from_redis_value(&json_str)?;
                Ok(Some(dto))
            }
            None => Ok(None),
        }
    }

    async fn set(&self, profile: &ProfileSummaryDto) -> Result<()> {
        let profile_id = ProfileId::try_new(&profile.profile_id).map_err(|e| {
            Error::validation("profile_id", format!("Invalid ProfileId in DTO: {}", e))
        })?;

        let key = self.build_key(&profile_id);
        let value = RedisProfileModel::to_redis_value(profile)?;

        // TTL de 24 heures pour les profils compacts
        let ttl = Duration::from_secs(86400);
        let expiration = Expiration::EX(ttl.as_secs() as i64);

        self.redis_pool
            .set::<(), _, _>(&key, value, Some(expiration), None, false)
            .await
            .map_err(|e| Error::database(format!("Redis SET profile failed: {}", e)))?;

        Ok(())
    }

    async fn invalidate(&self, profile_id: &ProfileId) -> Result<()> {
        let key = self.build_key(profile_id);

        let _: i64 = self
            .redis_pool
            .del(&key)
            .await
            .map_err(|e| Error::database(format!("Redis DEL profile failed: {}", e)))?;

        tracing::debug!(%key, "Profile cache eviction successfully processed");
        Ok(())
    }
}
