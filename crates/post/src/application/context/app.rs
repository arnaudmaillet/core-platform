// crates/post/src/application/context/app.rs

use crate::{
    context::{PostCommandContext, PostQueryContext},
    domain::repositories::PostRepository,
    resolvers::ProfileResolver,
};
use shared_kernel::{
    idempotency::IdempotencyRepository,
    types::{ProfileId, Region},
};
use std::sync::Arc;

#[derive(Clone)]
pub struct PostAppContext {
    post_repo: Arc<dyn PostRepository>,
    idempotency_repo: Arc<dyn IdempotencyRepository>,
    profile_resolver: Arc<dyn ProfileResolver>,
}

impl PostAppContext {
    pub fn new(
        post_repo: Arc<dyn PostRepository>,
        idempotency_repo: Arc<dyn IdempotencyRepository>,
        profile_resolver: Arc<dyn ProfileResolver>,
    ) -> Self {
        Self {
            post_repo,
            idempotency_repo,
            profile_resolver,
        }
    }

    pub fn new_stubbed(
        post_repo: Arc<dyn PostRepository>,
        idempotency_repo: Arc<dyn IdempotencyRepository>,
        profile_resolver: Arc<dyn ProfileResolver>,
    ) -> Self {
        Self {
            post_repo,
            idempotency_repo,
            profile_resolver,
        }
    }

    pub fn query(&self, region: Region) -> PostQueryContext {
        PostQueryContext::new(self.clone(), region)
    }

    pub fn command(&self, author_id: ProfileId, region: Region) -> PostCommandContext {
        PostCommandContext::new(self.clone(), author_id, region)
    }

    pub fn post_repo(&self) -> Arc<dyn PostRepository> {
        self.post_repo.clone()
    }

    pub fn idempotency_repo(&self) -> Arc<dyn IdempotencyRepository> {
        self.idempotency_repo.clone()
    }

    pub fn profile_resolver(&self) -> Arc<dyn ProfileResolver> {
        self.profile_resolver.clone()
    }
}
