// crates/post/src/application/context/app.rs

use crate::{domain::repositories::PostRepository, resolvers::ProfileResolver};
use shared_kernel::{
    environment::ClusterContext, idempotency::IdempotencyRepository, types::Region,
};
use std::sync::Arc;

#[derive(Clone)]
pub struct PostKernelCtx {
    post_repo: Arc<dyn PostRepository>,
    profile_resolver: Arc<dyn ProfileResolver>,
    cluster_ctx: ClusterContext,
}

impl PostKernelCtx {
    pub fn new(
        post_repo: Arc<dyn PostRepository>,
        profile_resolver: Arc<dyn ProfileResolver>,
        cluster_ctx: ClusterContext,
    ) -> Self {
        Self {
            post_repo,
            profile_resolver,
            cluster_ctx,
        }
    }

    pub fn server_region(&self) -> Region {
        self.cluster_ctx.region()
    }

    pub fn post_repo(&self) -> Arc<dyn PostRepository> {
        self.post_repo.clone()
    }

    pub fn profile_resolver(&self) -> Arc<dyn ProfileResolver> {
        self.profile_resolver.clone()
    }
}
