use crate::Post;
use async_trait::async_trait;
use shared_kernel::core::{PageQuery, PagedResult, Result};
use shared_kernel::types::{PostId, ProfileId};

#[async_trait]
pub trait PostReadRepository: Send + Sync {
    async fn find_by_id(&self, post_id: &PostId) -> Result<Option<Post>>;
    async fn find_by_author(
        &self,
        author_id: &ProfileId,
        query: PageQuery,
    ) -> Result<PagedResult<Post>>;
}
