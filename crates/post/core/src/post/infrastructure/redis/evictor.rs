use infra_fred::fred;
use infra_fred::fred::interfaces::KeysInterface;
use shared_kernel::core::{Error, Result};
use shared_kernel::types::PostId;

pub struct RedisPostEvictor {
    redis_pool: fred::clients::Pool,
}

impl RedisPostEvictor {
    pub fn new(redis_pool: fred::clients::Pool) -> Self {
        Self { redis_pool }
    }

    pub async fn evict(&self, post_id: &PostId) -> Result<()> {
        let key = format!("posts:atom:{}", post_id);
        
        let _: i64 = self
            .redis_pool
            .del(&key)
            .await
            .map_err(|e| Error::database(format!("Redis DEL post failed: {}", e)))?;

        Ok(())
    }
}