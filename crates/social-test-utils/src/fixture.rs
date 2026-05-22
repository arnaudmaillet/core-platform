// crates/social-test-utils/src/fixture.rs

use shared_kernel::cache::CacheRepositoryStub;
use shared_kernel::command::CommandBus;
use shared_kernel::idempotency::IdempotencyRepositoryStub;
use shared_kernel::types::{ProfileId, Region};
use std::sync::Arc;

// Application & Context
use social::commands::{FollowCommand, FollowHandler, UnfollowCommand, UnfollowHandler};
use social::context::{SocialAppContext, SocialContext};
use social::repositories::{
    CounterRepository, CounterRepositoryStub, RelationRepository, RelationRepositoryStub,
};

pub struct SocialTestFixture {
    bus: Arc<CommandBus>,
    app_ctx: SocialAppContext,
    social_ctx: SocialContext,

    // Accès direct aux stubs pour configurer l'état ou inspecter les mutations
    relation_repo: Arc<RelationRepositoryStub>,
    cache_counter_repo: Arc<CounterRepositoryStub>,
    db_counter_repo: Arc<CounterRepositoryStub>,
    idempotency_repo: Arc<IdempotencyRepositoryStub>,
}

impl SocialTestFixture {
    pub fn new() -> Self {
        // 1. Instanciation des stubs d'infrastructure
        let relation_repo = Arc::new(RelationRepositoryStub::new());
        let cache_counter_repo = Arc::new(CounterRepositoryStub::new(true)); // Comportement Redis (Cache Miss / Set Dirty)
        let db_counter_repo = Arc::new(CounterRepositoryStub::new(false)); // Comportement ScyllaDB
        let idempotency_repo = Arc::new(IdempotencyRepositoryStub::new());
        let cache = Arc::new(CacheRepositoryStub::new());

        // 2. Assemblage du SocialAppContext global
        let app_ctx = SocialAppContext::new(
            relation_repo.clone(),
            cache_counter_repo.clone(),
            db_counter_repo.clone(),
            idempotency_repo.clone(),
        );

        // 3. Configuration d'acteurs par défaut pour le test
        let region = Region::default();
        let default_target_id = ProfileId::generate(region);

        // Le contexte unifié centré sur notre cible
        let social_ctx = app_ctx.create_context(default_target_id, region);

        // 4. Enregistrement des Handlers d'écriture dans le CommandBus
        let mut bus = CommandBus::new(cache);
        bus.register::<SocialContext, FollowCommand, FollowHandler>(FollowHandler);
        bus.register::<SocialContext, UnfollowCommand, UnfollowHandler>(UnfollowHandler);

        Self {
            bus: Arc::new(bus),
            app_ctx,
            social_ctx,
            relation_repo,
            cache_counter_repo,
            db_counter_repo,
            idempotency_repo,
        }
    }

    // --- ACCESSEURS ---

    pub fn bus(&self) -> Arc<CommandBus> {
        self.bus.clone()
    }

    pub fn app_ctx(&self) -> &SocialAppContext {
        &self.app_ctx
    }

    pub fn social_ctx(&self) -> &SocialContext {
        &self.social_ctx
    }

    pub fn target_profile_id(&self) -> ProfileId {
        ProfileId::generate(self.region())
    }

    pub fn region(&self) -> Region {
        self.social_ctx.region()
    }

    pub fn relation_repo(&self) -> &RelationRepositoryStub {
        &self.relation_repo
    }

    pub fn cache_counter_repo(&self) -> &CounterRepositoryStub {
        &self.cache_counter_repo
    }

    pub fn db_counter_repo(&self) -> &CounterRepositoryStub {
        &self.db_counter_repo
    }

    pub fn idempotency_repo(&self) -> &IdempotencyRepositoryStub {
        &self.idempotency_repo
    }

    // --- HELPERS : GIVEN (Configuration de l'état initial) ---

    pub fn given_existing_relation(&self, follower_id: ProfileId, following_id: ProfileId) {
        self.relation_repo.seed_relation(follower_id, following_id);
    }

    pub fn given_initial_counters(&self, profile_id: ProfileId, followers: u64, following: u64) {
        self.cache_counter_repo
            .seed_counters(profile_id, followers, following);
        self.db_counter_repo
            .seed_counters(profile_id, followers, following);
    }

    // --- HELPERS : THEN (Assertions fluides) ---

    pub async fn assert_relation_exists(&self, follower_id: ProfileId, following_id: ProfileId) {
        let exists = self
            .relation_repo
            .is_following(follower_id, following_id)
            .await
            .unwrap();
        assert!(
            exists,
            "La relation de suivi [{} -> {}] aurait dû exister",
            follower_id, following_id
        );
    }

    pub async fn assert_relation_does_not_exist(
        &self,
        follower_id: ProfileId,
        following_id: ProfileId,
    ) {
        let exists = self
            .relation_repo
            .is_following(follower_id, following_id)
            .await
            .unwrap();
        assert!(
            !exists,
            "La relation de suivi [{} -> {}] n'aurait PAS dû exister",
            follower_id, following_id
        );
    }

    pub async fn assert_counters_values(
        &self,
        profile_id: ProfileId,
        expected_followers: u64,
        expected_following: u64,
    ) {
        let redis_res = self
            .cache_counter_repo
            .get_counters(profile_id)
            .await
            .unwrap();
        assert_eq!(
            redis_res.followers_count().value(),
            expected_followers,
            "Redis followers count mismatch"
        );
        assert_eq!(
            redis_res.following_count().value(),
            expected_following,
            "Redis following count mismatch"
        );

        assert!(
            self.cache_counter_repo.is_profile_dirty(&profile_id),
            "Le profil {} aurait dû être marqué comme DIRTY dans Redis",
            profile_id
        );
    }
}
