// crates/post/src/application/cache/profile_cache_repository.rs

use async_trait::async_trait;
use shared_kernel::core::Result;
use shared_kernel::types::ProfileId;
use shared_proto::profile::v1::ProfileSummaryDto;

#[async_trait]
pub trait ProfileCacheRepository: Send + Sync {
    /// Tente de récupérer le profil compact depuis Redis
    async fn get(&self, profile_id: &ProfileId) -> Result<Option<ProfileSummaryDto>>;

    /// Hydrate le cache Redis (généralement après un Scylla Miss) avec un TTL (ex: 24h)
    async fn set(&self, profile: &ProfileSummaryDto) -> Result<()>;

    /// Invalide (supprime) la clé de cache (généralement appelé par le worker Kafka)
    async fn invalidate(&self, profile_id: &ProfileId) -> Result<()>;
}