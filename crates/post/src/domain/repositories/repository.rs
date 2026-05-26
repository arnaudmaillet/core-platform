// crates/post/src/domain/repositories/post_repository.rs

use async_trait::async_trait;
use shared_kernel::core::{PageQuery, PagedResult, Result};
use shared_kernel::types::{PostId, ProfileId, Region};

use crate::domain::entities::Post;

#[async_trait]
pub trait PostRepository: Send + Sync {
    async fn save(&self, region: Region, post: &Post) -> Result<()>;
    async fn find_by_id(&self, region: Region, post_id: &PostId) -> Result<Option<Post>>;
    async fn find_by_author(
        &self,
        region: Region,
        author_id: &ProfileId,
        query: PageQuery,
    ) -> Result<PagedResult<Post>>;

    async fn delete(&self, region: Region, post_id: &PostId, author_id: &ProfileId) -> Result<()>;
}
