// crates/post/core/src/post/infrastructure/decorators/cached_read.rs

use crate::Post;
use crate::post::repositories::PostReadRepository;
use async_trait::async_trait;
use shared_kernel::cache::{CacheRepository, CacheRepositoryExt};
use shared_kernel::core::{PageQuery, PagedResult, Result};
use shared_kernel::types::{PostId, ProfileId};
use std::time::Duration;

pub struct CachedPostReadRepository<R, C>
where
    R: PostReadRepository,
    C: CacheRepository,
{
    inner: R,
    cache: C,
}

impl<R, C> CachedPostReadRepository<R, C>
where
    R: PostReadRepository,
    C: CacheRepository,
{
    pub fn new(inner: R, cache: C) -> Self {
        Self { inner, cache }
    }

    fn build_key(&self, post_id: &PostId) -> String {
        format!("posts:atom:{}", post_id)
    }
}

#[async_trait]
impl<R, C> PostReadRepository for CachedPostReadRepository<R, C>
where
    R: PostReadRepository + Sync + Send,
    C: CacheRepository + Sync + Send,
{
    async fn find_by_id(&self, post_id: &PostId) -> Result<Option<Post>> {
        let key = self.build_key(post_id);

        if let Some(post) = self.cache.get_obj::<Post>(&key).await? {
            return Ok(Some(post));
        }

        if let Some(post) = self.inner.find_by_id(post_id).await? {
            // 3. Hydratation : Sérialisation et mise en cache asynchrone (TTL 12h)
            let ttl = Duration::from_secs(43200);
            self.cache.set_obj(&key, &post, Some(ttl)).await?;

            return Ok(Some(post));
        }

        Ok(None)
    }

    async fn find_by_author(
        &self,
        author_id: &ProfileId,
        query: PageQuery,
    ) -> Result<PagedResult<Post>> {
        // Pas de cache Redis sur les collections paginées mouvantes (Timeline).
        // On délègue directement à ScyllaDB.
        self.inner.find_by_author(author_id, query).await
    }
}
