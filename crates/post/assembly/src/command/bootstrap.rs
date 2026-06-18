// crates/post/assembly/src/command/bootstrap.rs

use crate::PostCommandContainer;
use infra_fred::RedisCacheRepository;
use infra_scylla::scylla::client;
use post::{
    CachedPostReadRepository, ChangeVisibilityCommand, ChangeVisibilityHandler, CreatePostCommand,
    CreatePostHandler, DeletePostCommand, DeletePostHandler, PostCommandCtx, PostKernelCtx,
    ScyllaPostReadRepository, ScyllaPostWriteRepository, ToggleCommentsCommand,
    ToggleCommentsHandler, UpdateCaptionCommand, UpdateCaptionHandler,
};
use shared_kernel::{command::CommandBus, core::Error, environment::ClusterContext};
use std::sync::Arc;

pub struct PostCommandAssembly;

impl PostCommandAssembly {
    pub async fn bootstrap(
        session: Arc<client::session::Session>,
        cache_repo: RedisCacheRepository,
        keyspace_name: String,
        cluster_ctx: ClusterContext,
        bus: CommandBus,
    ) -> Result<PostCommandContainer, Error> {
        let scylla_post_write_repo =
            ScyllaPostWriteRepository::new(session.clone(), keyspace_name.clone()).await?;
        let post_write_repo = Arc::new(scylla_post_write_repo);

        let scylla_post_read_repo =
            ScyllaPostReadRepository::new(session.clone(), keyspace_name.clone()).await?;
        let cached_post_read_repo =
            CachedPostReadRepository::new(scylla_post_read_repo, cache_repo.clone());
        let post_read_repo = Arc::new(cached_post_read_repo);

        let kernel_ctx = PostKernelCtx::new(post_read_repo, post_write_repo, cluster_ctx);

        let managed_bus = Self::register_handlers(bus);

        Ok(PostCommandContainer {
            bus: managed_bus,
            kernel_ctx,
        })
    }

    pub fn register_handlers(mut bus: CommandBus) -> CommandBus {
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

        bus
    }
}
