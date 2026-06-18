use crate::{cache::CacheRepository, core::Result};
use async_trait::async_trait;

#[async_trait]
pub trait CacheInvalidator: Send + Sync {
    async fn invalidate(&self, key: &str) -> Result<()>;
}

// Blanket implementation
// Tout type T (comme ton RedisCacheRepository) qui sait faire du CacheRepository
// sait AUTOMATIQUEMENT faire du CacheInvalidator sans coder de nouvelle structure.
#[async_trait]
impl<T> CacheInvalidator for T
where
    T: CacheRepository + Send + Sync,
{
    async fn invalidate(&self, key: &str) -> Result<()> {
        self.delete(key).await
    }
}
