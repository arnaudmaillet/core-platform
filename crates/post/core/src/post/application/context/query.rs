// crates/post/core/src/post/application/context/query.rs

use crate::{Post, post::application::context::PostKernelCtx};
use shared_kernel::{
    core::{PageQuery, PagedResult, Result},
    types::{PostId, ProfileId, Region},
};

#[derive(Clone)]
pub struct PostQueryCtx {
    kernel: PostKernelCtx,
    region_query: Region,
}

impl PostQueryCtx {
    pub fn new(kernel: PostKernelCtx, region_query: Region) -> Self {
        Self {
            kernel,
            region_query,
        }
    }

    pub fn region(&self) -> Region {
        self.region_query
    }

    pub async fn find_by_id(&self, post_id: &PostId) -> Result<Option<Post>> {
        self.kernel.read_repo().find_by_id(post_id).await
    }

    pub async fn find_by_author(
        &self,
        author_id: &ProfileId,
        query: PageQuery,
    ) -> Result<PagedResult<Post>> {
        self.kernel
            .read_repo()
            .find_by_author(author_id, query)
            .await
    }
}
