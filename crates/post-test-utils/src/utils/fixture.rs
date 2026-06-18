// crates/post/src/application/utils/fixture.rs

use std::sync::Arc;

use crate::resolvers::ProfileResolverStub;
use crate::stores::PostStoreStub;
use post::PostServiceBuilder;
use post::builders::PostBuilder;
use post::context::{PostCommandCtx, PostKernelCtx, PostQueryCtx};
use post::entities::Post;
use post::repositories::PostRepository;
use post::types::{Caption, VisibilityLevel};
use shared_kernel::command::CommandBus;
use shared_kernel::environment::ClusterContext;
use shared_kernel::types::{PostId, PostType, ProfileId, Region};
use shared_kernel_test_utils::repositories::CacheRepositoryStub;
use shared_kernel_test_utils::repositories::IdempotencyRepositoryStub;

pub struct PostTestFixture {
    bus: CommandBus,
    author_id: ProfileId,
    post_id: PostId,

    kernel_ctx: PostKernelCtx,
    command_ctx: PostCommandCtx,
    query_ctx: PostQueryCtx,
    cluster_ctx: ClusterContext,

    post_repo: Arc<PostStoreStub>,
    idempotency_repo: Arc<IdempotencyRepositoryStub>,
    profile_resolver: Arc<ProfileResolverStub>,
}

impl PostTestFixture {
    pub fn new() -> Self {
        let post_repo = Arc::new(PostStoreStub::new());
        let idempotency_repo = Arc::new(IdempotencyRepositoryStub::new());
        let cache_repo = Arc::new(CacheRepositoryStub::new());
        let profile_resolver = Arc::new(ProfileResolverStub::new());

        let cluster_ctx = ClusterContext::default();

        let service =
            PostServiceBuilder::new(post_repo.clone(), profile_resolver.clone(), cluster_ctx);

        let author_id = ProfileId::generate();
        let post_id = PostId::generate();

        let kernel_ctx = service.build_kernel_ctx();
        let command_ctx = PostCommandCtx::new(kernel_ctx.clone(), author_id, cluster_ctx.region());
        let query_ctx = PostQueryCtx::new(kernel_ctx.clone(), cluster_ctx.region());

        let mut bus = CommandBus::new(Some(idempotency_repo.clone()), Some(cache_repo));

        service.register_handlers(&mut bus);

        Self {
            bus,
            author_id,
            post_id,
            kernel_ctx,
            command_ctx,
            query_ctx,
            cluster_ctx,
            post_repo,
            idempotency_repo,
            profile_resolver,
        }
    }

    // --- Accesseurs unifiés (Alignés sur Account) ---

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

    pub fn query_ctx(&self) -> &PostQueryCtx {
        &self.query_ctx
    }

    pub fn cluster_ctx(&self) -> &ClusterContext {
        &self.cluster_ctx
    }

    pub fn idempotency_repo(&self) -> &IdempotencyRepositoryStub {
        &self.idempotency_repo
    }

    pub fn profile_resolver(&self) -> &ProfileResolverStub {
        &self.profile_resolver
    }

    pub fn post_assertions(&self) -> &PostStoreStub {
        &self.post_repo
    }

    pub async fn given_post(&self, post: &Post) {
        self.post_repo
            .save(post)
            .await
            .expect("Le setup de l'état initial via le stub Scylla a échoué");
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
