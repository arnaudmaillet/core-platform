// crates/post/src/application/builder.rs

use infra_scylla::scylla::client::session::Session as ScyllaSession;
use std::sync::Arc;

// Post Domain & Application
use crate::application::context::{PostAppContext, PostCommandContext};
use crate::commands::{
    ChangeVisibilityCommand, ChangeVisibilityHandler, CreatePostCommand, CreatePostHandler,
    DeletePostCommand, DeletePostHandler, ToggleCommentsCommand, ToggleCommentsHandler,
    UpdateCaptionCommand, UpdateCaptionHandler,
};
use crate::repositories::PostRepository;
use crate::repositories_impl::{CachePostRepository, ScyllaPostRepository};
use crate::resolvers::ProfileResolver;

// Shared Kernel
use shared_kernel::{
    cache::CacheRepository, command::CommandBus, idempotency::IdempotencyRepository,
};

pub struct PostServiceBuilder {
    keyspace: String,
    scylla_session: Arc<ScyllaSession>,
    redis_cache_repo: Arc<dyn CacheRepository>,
    idempotency_repo: Arc<dyn IdempotencyRepository>,
    profile_resolver: Arc<dyn ProfileResolver>,
}

impl PostServiceBuilder {
    pub fn new(
        keyspace: String,
        scylla_session: Arc<ScyllaSession>,
        redis_cache_repo: Arc<dyn CacheRepository>,
        idempotency_repo: Arc<dyn IdempotencyRepository>,
        profile_resolver: Arc<dyn ProfileResolver>,
    ) -> Self {
        Self {
            keyspace,
            scylla_session,
            redis_cache_repo,
            idempotency_repo,
            profile_resolver,
        }
    }

    pub async fn build_context(
        &self,
    ) -> Result<Arc<PostAppContext>, infra_scylla::scylla::errors::PrepareError> {
        let scylla_repo =
            ScyllaPostRepository::new(self.scylla_session.clone(), &self.keyspace).await?;
        let scylla_repo_with_cache: Arc<dyn PostRepository> = Arc::new(CachePostRepository::new(
            scylla_repo,
            self.redis_cache_repo.clone(),
        ));

        Ok(Arc::new(PostAppContext::new(
            scylla_repo_with_cache,
            self.idempotency_repo.clone(),
            self.profile_resolver.clone(),
        )))
    }

    pub fn build_command_bus(&self) -> Arc<CommandBus> {
        let mut bus = CommandBus::new(self.redis_cache_repo.clone());
        bus.register::<PostCommandContext, CreatePostCommand, CreatePostHandler>(CreatePostHandler);
        bus.register::<PostCommandContext, UpdateCaptionCommand, UpdateCaptionHandler>(
            UpdateCaptionHandler,
        );
        bus.register::<PostCommandContext, ToggleCommentsCommand, ToggleCommentsHandler>(
            ToggleCommentsHandler,
        );
        bus.register::<PostCommandContext, ChangeVisibilityCommand, ChangeVisibilityHandler>(
            ChangeVisibilityHandler,
        );
        bus.register::<PostCommandContext, DeletePostCommand, DeletePostHandler>(DeletePostHandler);
        Arc::new(bus)
    }
}
