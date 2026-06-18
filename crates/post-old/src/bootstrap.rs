// crates/post/src/application/builder.rs

use crate::application::context::{PostCommandCtx, PostKernelCtx};
use crate::commands::{
    ChangeVisibilityCommand, ChangeVisibilityHandler, CreatePostCommand, CreatePostHandler,
    DeletePostCommand, DeletePostHandler, ToggleCommentsCommand, ToggleCommentsHandler,
    UpdateCaptionCommand, UpdateCaptionHandler,
};
use crate::repositories::PostRepository;
use crate::resolvers::ProfileResolver;
use std::sync::Arc;

use shared_kernel::command::CommandBus;
use shared_kernel::environment::ClusterContext;

pub struct PostServiceBuilder {
    post_repo: Arc<dyn PostRepository>,
    profile_resolver: Arc<dyn ProfileResolver>,
    cluster_ctx: ClusterContext,
}

impl PostServiceBuilder {
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

    pub fn build_kernel_ctx(&self) -> PostKernelCtx {
        PostKernelCtx::new(
            self.post_repo.clone(),
            self.profile_resolver.clone(),
            self.cluster_ctx,
        )
    }

    pub fn register_handlers(&self, bus: &mut CommandBus) {
        bus.register::<PostCommandCtx, CreatePostCommand, CreatePostHandler>(CreatePostHandler);
        bus.register::<PostCommandCtx, UpdateCaptionCommand, UpdateCaptionHandler>(
            UpdateCaptionHandler,
        );
        bus.register::<PostCommandCtx, ToggleCommentsCommand, ToggleCommentsHandler>(
            ToggleCommentsHandler,
        );
        bus.register::<PostCommandCtx, ChangeVisibilityCommand, ChangeVisibilityHandler>(
            ChangeVisibilityHandler,
        );
        bus.register::<PostCommandCtx, DeletePostCommand, DeletePostHandler>(DeletePostHandler);
    }
}
