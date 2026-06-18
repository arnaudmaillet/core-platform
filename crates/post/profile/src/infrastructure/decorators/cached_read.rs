use crate::{ProfileReadProjection, ProjectedProfile};
use async_trait::async_trait;
use shared_kernel::cache::{CacheRepository, CacheRepositoryExt};
use shared_kernel::core::Result;
use shared_kernel::types::ProfileId;

pub struct CachedProfileReadRepository<P, C>
where
    P: ProfileReadProjection,
    C: CacheRepository,
{
    inner: P,
    cache: C,
}

impl<P, C> CachedProfileReadRepository<P, C>
where
    P: ProfileReadProjection,
    C: CacheRepository,
{
    pub fn new(inner: P, cache: C) -> Self {
        Self { inner, cache }
    }

    fn build_key(&self, profile_id: &ProfileId) -> String {
        format!("profiles:compact:{}", profile_id)
    }
}

#[async_trait]
impl<P, C> ProfileReadProjection for CachedProfileReadRepository<P, C>
where
    P: ProfileReadProjection + Sync + Send,
    C: CacheRepository + Sync + Send,
{
    async fn find_by_id(&self, profile_id: &ProfileId) -> Result<Option<ProjectedProfile>> {
        let key = self.build_key(profile_id);

        if let Some(profile_dto) = self.cache.get_obj::<ProjectedProfile>(&key).await? {
            return Ok(Some(profile_dto));
        }

        if let Some(profile_dto) = self.inner.find_by_id(profile_id).await? {
            // 3. Hydratation : Sérialisation et stockage asynchrone (TTL de 24 heures)
            let ttl = std::time::Duration::from_secs(86400);
            self.cache.set_obj(&key, &profile_dto, Some(ttl)).await?;

            return Ok(Some(profile_dto));
        }

        Ok(None)
    }
}
