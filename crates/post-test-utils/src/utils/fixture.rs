// crates/post/src/application/utils/fixture.rs

use std::sync::Arc;

use crate::repositories::PostRepositoryStub;
use crate::resolvers::ProfileResolverStub;
use post::builders::PostBuilder;
use post::commands::{
    ChangeVisibilityCommand, ChangeVisibilityHandler, CreatePostCommand, CreatePostHandler,
    DeletePostCommand, DeletePostHandler, ToggleCommentsCommand, ToggleCommentsHandler,
    UpdateCaptionCommand, UpdateCaptionHandler,
};
use post::context::{PostAppContext, PostCommandContext, PostQueryContext};
use post::entities::Post;
use post::repositories::PostRepository;
use post::types::{Caption, VisibilityLevel};
use shared_kernel::command::CommandBus;
use shared_kernel::core::Result;
use shared_kernel::types::{PostId, PostType, ProfileId, Region};
use shared_kernel_test_utils::repositories::CacheRepositoryStub;
use shared_kernel_test_utils::repositories::IdempotencyRepositoryStub;

pub struct PostTestFixture {
    bus: Arc<CommandBus>,
    app_ctx: PostAppContext,
    author_id: ProfileId,
    post_id: PostId,
    region: Region,
    post_repo: Arc<PostRepositoryStub>,
    idempotency_repo: Arc<IdempotencyRepositoryStub>,
    profile_resolver: Arc<ProfileResolverStub>,
}

impl PostTestFixture {
    pub fn new() -> Self {
        let post_repo = Arc::new(PostRepositoryStub::new());
        let idempotency_repo = Arc::new(IdempotencyRepositoryStub::new());
        let cache = Arc::new(CacheRepositoryStub::new());
        let profile_resolver = Arc::new(ProfileResolverStub::new());

        // Utilisation du constructeur de stub
        let app_ctx = PostAppContext::new_stubbed(
            post_repo.clone(),
            idempotency_repo.clone(),
            profile_resolver.clone(),
        );

        // Configuration par défaut pour isoler les régions dans les tests
        let region = Region::default();
        let author_id = ProfileId::generate();
        let post_id = PostId::generate();

        let mut bus = CommandBus::new(cache);

        // --- Enregistrement des Handlers de Commandes avec PostCommandContext ---
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

        Self {
            bus: Arc::new(bus),
            app_ctx,
            author_id,
            post_id,
            region,
            post_repo,
            idempotency_repo,
            profile_resolver,
        }
    }

    // --- Accesseurs ---

    pub fn bus(&self) -> Arc<CommandBus> {
        self.bus.clone()
    }

    pub fn app_ctx(&self) -> &PostAppContext {
        &self.app_ctx
    }

    pub fn author_id(&self) -> ProfileId {
        self.author_id
    }

    pub fn post_id(&self) -> PostId {
        self.post_id
    }

    pub fn region(&self) -> Region {
        self.region
    }

    pub fn writer_ctx(&self) -> PostCommandContext {
        self.app_ctx.command(self.author_id, self.region)
    }

    pub fn reader_ctx(&self) -> PostQueryContext {
        self.app_ctx.query(self.region)
    }

    pub fn post_repo(&self) -> &PostRepositoryStub {
        &self.post_repo
    }

    pub fn idempotency_repo(&self) -> &IdempotencyRepositoryStub {
        &self.idempotency_repo
    }

    pub fn profile_resolver(&self) -> &ProfileResolverStub {
        &self.profile_resolver
    }

    // --- Helpers pour l'organisation des scénarios (`Given`) ---

    /// Prépare un post pré-existant directement dans l'infrastructure de mock
    pub async fn given_post(&self, post: &Post) {
        self.post_repo
            .save(self.region, post)
            .await
            .expect("Le setup de l'état initial via le stub a échoué");
    }

    /// Factory pré-configurée pour générer un builder valide aligné sur la fixture courante
    pub fn builder(&self, raw_caption: &str) -> PostBuilder {
        Post::builder(
            self.post_id(),
            self.author_id(),
            PostType::Text,
            VisibilityLevel::Public,
        )
        .with_caption(Caption::from_raw(raw_caption.to_string()))
    }

    // --- Assertions réutilisables ---

    pub async fn assert_post<F>(&self, check: F) -> Result<()>
    where
        F: FnOnce(&Post),
    {
        let saved_option = self
            .post_repo
            .find_by_id(self.region, &self.post_id())
            .await?;

        let saved = saved_option.ok_or_else(|| {
            shared_kernel::core::Error::not_found("Post", self.post_id().to_string())
        })?;

        check(&saved);
        Ok(())
    }

    pub fn clone_with_post_id(&self, new_post_id: PostId) -> Self {
        let author_id = ProfileId::generate();

        Self {
            bus: self.bus.clone(),
            app_ctx: self.app_ctx.clone(),
            author_id,
            post_id: new_post_id,
            region: self.region,
            post_repo: self.post_repo.clone(),
            idempotency_repo: self.idempotency_repo.clone(),
            profile_resolver: self.profile_resolver.clone(),
        }
    }
}
