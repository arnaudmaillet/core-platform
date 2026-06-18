// crates/post/profile/src/infrastructure/redis/evictor.rs

use infra_fred::fred;
use infra_fred::fred::interfaces::KeysInterface;
use shared_kernel::core::{Error, Result};
use shared_kernel::types::ProfileId;

pub struct RedisProfileEvictor {
    redis_pool: fred::clients::Pool,
}

impl RedisProfileEvictor {
    pub fn new(redis_pool: fred::clients::Pool) -> Self {
        Self { redis_pool }
    }

    pub async fn evict(&self, profile_id: &ProfileId) -> Result<()> {
        let key = format!("profiles:compact:{}", profile_id);

        let _: i64 = self
            .redis_pool
            .del(&key)
            .await
            .map_err(|e| Error::database(format!("Redis DEL profile failed: {}", e)))?;

        tracing::debug!(%key, "Profile cache asynchronously evicted by CDC worker");
        Ok(())
    }
}
