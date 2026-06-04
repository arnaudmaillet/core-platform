// crates/post/src/application/context/query.rs

use shared_kernel::{
    core::{PageQuery, PagedResult, Result},
    types::{PostId, ProfileId, Region},
};

use crate::{context::PostAppContext, entities::Post};

#[derive(Clone)]
pub struct PostQueryContext {
    app: PostAppContext,
    region: Region,
}

impl PostQueryContext {
    pub fn new(app: PostAppContext, region: Region) -> Self {
        Self { app, region }
    }

    pub fn region(&self) -> Region {
        self.region
    }

    pub async fn find_by_id(&self, post_id: &PostId) -> Result<Option<Post>> {
        self.app.post_repo().find_by_id(self.region, post_id).await
    }

    pub async fn find_by_author(
        &self,
        author_id: &ProfileId,
        query: PageQuery,
    ) -> Result<PagedResult<Post>> {
        self.app
            .post_repo()
            .find_by_author(self.region, author_id, query)
            .await
    }
}
