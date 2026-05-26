use async_trait::async_trait;
use shared_kernel::cache::CacheRepository;
use shared_kernel::core::{PageQuery, PagedResult, Result};
use shared_kernel::types::{PostId, ProfileId, Region};
use std::sync::Arc;
use std::time::Duration;

use crate::domain::entities::Post;
use crate::domain::repositories::PostRepository;
use crate::repositories_impl::ScyllaPostRepository;

pub struct CachePostRepository {
    scylla_repo: ScyllaPostRepository,
    cache_repo: Arc<dyn CacheRepository>,
}

impl CachePostRepository {
    pub fn new(scylla_repo: ScyllaPostRepository, cache_repo: Arc<dyn CacheRepository>) -> Self {
        Self {
            scylla_repo,
            cache_repo,
        }
    }
    fn cache_key(&self, region: Region, post_id: &PostId) -> String {
        format!("posts:{}:{}", region, post_id)
    }
}

#[async_trait]
impl PostRepository for CachePostRepository {
    async fn find_by_id(&self, region: Region, post_id: &PostId) -> Result<Option<Post>> {
        let key = self.cache_key(region, post_id);

        if let Ok(Some(cached_post)) = self.cache_repo.get(&key).await {
            if let Ok(post) = serde_json::from_str::<Post>(&cached_post) {
                tracing::info!(key = %key, "CachedPostRepository: Cache Hit");
                return Ok(Some(post));
            }
        }

        tracing::info!(key = %key, "CachedPostRepository: Cache Miss, fetching from ScyllaDB");
        let post_from_bd = self.scylla_repo.find_by_id(region, post_id).await?;

        if let Some(ref post) = post_from_bd {
            if let Ok(serialized) = serde_json::to_string(post) {
                let _ = self
                    .cache_repo
                    .set(&key, &serialized, Some(Duration::from_secs(3600)))
                    .await;
            }
        }

        Ok(post_from_bd)
    }

    async fn save(&self, region: Region, post: &Post) -> Result<()> {
        self.scylla_repo.save(region, post).await
        // Note : L'invalidation du cache Redis (`cache.delete`) est gérée automatiquement
        // par le CommandBus en fin d'exécution de la commande grâce au `cache_key()` de celle-ci.
    }

    async fn find_by_author(
        &self,
        region: Region,
        author_id: &ProfileId,
        query: PageQuery,
    ) -> Result<PagedResult<Post>> {
        self.scylla_repo
            .find_by_author(region, author_id, query)
            .await
    }

    async fn delete(&self, region: Region, post_id: &PostId, author_id: &ProfileId) -> Result<()> {
        let key = self.cache_key(region, post_id);
        self.scylla_repo.delete(region, post_id, author_id).await?;
        let _ = self.cache_repo.delete(&key).await;

        Ok(())
    }
}
