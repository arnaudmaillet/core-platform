use post::{Caption, Post, PostBuilder, VisibilityLevel};
use post::{PostCommandCtx, PostKernelCtx};
use post::{PostReadRepositoryStub, PostWriteRepository, PostWriteRepositoryStub};
use post_assembly::PostCommandAssembly;
use post_profile::ProfileProjectionStub;
use shared_kernel::command::CommandBus;
use shared_kernel::environment::ClusterContext;
use shared_kernel::types::{PostId, PostType, ProfileId, Region};
use shared_kernel_test_utils::repositories::IdempotencyRepositoryStub;
use std::sync::Arc;

pub struct PostTestFixture {
    bus: CommandBus,
    author_id: ProfileId,
    post_id: PostId,

    kernel_ctx: PostKernelCtx,
    command_ctx: PostCommandCtx,
    cluster_ctx: ClusterContext,

    post_write_repo: Arc<PostWriteRepositoryStub>,
    post_read_repo: Arc<PostReadRepositoryStub>,
    idempotency_repo: Arc<IdempotencyRepositoryStub>,
    profile_projection_stub: Arc<ProfileProjectionStub>,
}

impl PostTestFixture {
    pub fn new() -> Self {
        // 1. Initialisation de l'infrastructure simulée en RAM (Stubs)
        let post_write_repo = Arc::new(PostWriteRepositoryStub::new());
        let post_read_repo = Arc::new(PostReadRepositoryStub::new());
        let idempotency_repo = Arc::new(IdempotencyRepositoryStub::new());
        let profile_projection_stub = Arc::new(ProfileProjectionStub::new());
        let cluster_ctx = ClusterContext::default();
        let author_id = ProfileId::generate();
        let post_id = PostId::generate();

        // Bus de commande local configuré avec le stub de cache
        let command_bus = CommandBus::new(None, None);

        // 2. Bootstrap de l'Assembly pour les tests
        // Pour que ce soit compatible, PostAssembly::bootstrap doit accepter des Arc<dyn ...>
        // ou être configuré pour recevoir des stubs en mode test.
        let kernel_ctx = PostKernelCtx::new(
            post_read_repo.clone(),
            post_write_repo.clone(),
            cluster_ctx.clone(),
        );

        let command_ctx = PostCommandCtx::new(kernel_ctx.clone(), author_id, cluster_ctx.region());

        // Note : Si ton `PostAssembly` gère l'enregistrement des handlers sur le bus,
        // tu passes le kernel_ctx créé avec tes stubs au bootstrap du bus.
        let bus = PostCommandAssembly::register_handlers(command_bus);

        Self {
            bus,
            author_id,
            post_id,
            kernel_ctx,
            command_ctx,
            cluster_ctx,
            post_write_repo,
            post_read_repo,
            idempotency_repo,
            profile_projection_stub,
        }
    }

    // --- Accesseurs mis à jour ---

    pub fn bus(&self) -> &CommandBus {
        &self.bus
    }

    pub fn author_id(&self) -> ProfileId {
        self.author_id
    }

    pub fn post_id(&self) -> PostId {
        self.post_id
    }

    pub fn server_region(&self) -> Region {
        self.cluster_ctx.region()
    }

    pub fn kernel_ctx(&self) -> &PostKernelCtx {
        &self.kernel_ctx
    }

    pub fn command_ctx(&self) -> &PostCommandCtx {
        &self.command_ctx
    }

    pub fn cluster_ctx(&self) -> &ClusterContext {
        &self.cluster_ctx
    }

    pub fn idempotency_repo(&self) -> &IdempotencyRepositoryStub {
        &self.idempotency_repo
    }

    // Renommé pour correspondre au stub unique
    pub fn profile_projection_stub(&self) -> &ProfileProjectionStub {
        &self.profile_projection_stub
    }

    pub fn post_assertions(&self) -> &PostWriteRepositoryStub {
        &self.post_write_repo
    }

    pub async fn given_post(&self, post: &Post) {
        self.post_write_repo
            .save(post)
            .await
            .expect("Le setup de l'état initial d'écriture a échoué");

        self.post_read_repo.feed(post.clone());
    }

    pub fn builder(&self, raw_caption: &str) -> PostBuilder {
        Post::builder(
            self.post_id(),
            self.author_id(),
            PostType::Text,
            VisibilityLevel::Public,
        )
        .with_caption(Caption::from_raw(raw_caption.to_string()))
    }
}
