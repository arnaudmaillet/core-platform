// crates/social-test-utils/src/fixture.rs

use shared_kernel::command::CommandBus;
use shared_kernel::types::{ProfileId, Region};
use shared_kernel_test_utils::repositories::CacheRepositoryStub;
use shared_kernel_test_utils::repositories::IdempotencyRepositoryStub;
use social::commands::{FollowCommand, FollowHandler, UnfollowCommand, UnfollowHandler};
use social::context::{SocialAppContext, SocialCommandContext, SocialQueryContext};
use social::repositories::{CounterRepository, RelationRepository};
use std::sync::Arc;

use crate::repositories::{CounterRepositoryStub, RelationRepositoryStub};

pub struct SocialTestFixture {
    bus: Arc<CommandBus>,
    region: Region,
    target_profile_id: ProfileId,
    app_ctx: SocialAppContext,
    command_ctx: SocialCommandContext,
    query_ctx: SocialQueryContext,
    relation_repo: Arc<RelationRepositoryStub>,
    cache_counter_repo: Arc<CounterRepositoryStub>,
    db_counter_repo: Arc<CounterRepositoryStub>,
    idempotency_repo: Arc<IdempotencyRepositoryStub>,
}

impl SocialTestFixture {
    pub fn new() -> Self {
        let relation_repo = Arc::new(RelationRepositoryStub::new());
        let cache_counter_repo = Arc::new(CounterRepositoryStub::new(true)); // Comportement Redis (Cache Miss / Set Dirty)
        let db_counter_repo = Arc::new(CounterRepositoryStub::new(false)); // Comportement ScyllaDB
        let idempotency_repo = Arc::new(IdempotencyRepositoryStub::new());
        let cache = Arc::new(CacheRepositoryStub::new());

        let app_ctx = SocialAppContext::new(
            relation_repo.clone(),
            cache_counter_repo.clone(),
            db_counter_repo.clone(),
            idempotency_repo.clone(),
        );

        let region = Region::default();
        let target_profile_id = ProfileId::generate();

        let command_ctx = app_ctx.command(target_profile_id, region);
        let query_ctx = app_ctx.query(region);

        let mut bus = CommandBus::new(cache);
        bus.register::<SocialCommandContext, FollowCommand, FollowHandler>(FollowHandler);
        bus.register::<SocialCommandContext, UnfollowCommand, UnfollowHandler>(UnfollowHandler);

        Self {
            bus: Arc::new(bus),
            region,
            target_profile_id,
            app_ctx,
            command_ctx,
            query_ctx,
            relation_repo,
            cache_counter_repo,
            db_counter_repo,
            idempotency_repo,
        }
    }

    pub fn bus(&self) -> Arc<CommandBus> {
        self.bus.clone()
    }

    pub fn app_ctx(&self) -> &SocialAppContext {
        &self.app_ctx
    }

    pub fn command_ctx(&self) -> &SocialCommandContext {
        &self.command_ctx
    }

    pub fn query_ctx(&self) -> &SocialQueryContext {
        &self.query_ctx
    }

    pub fn target_profile_id(&self) -> ProfileId {
        self.target_profile_id
    }

    pub fn region(&self) -> Region {
        self.region
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

    pub fn given_existing_relation(&self, follower_id: ProfileId, following_id: ProfileId) {
        self.relation_repo.seed_relation(follower_id, following_id);
    }

    pub fn given_initial_counters(&self, profile_id: ProfileId, followers: u64, following: u64) {
        self.cache_counter_repo
            .seed_counters(profile_id, followers, following);
        self.db_counter_repo
            .seed_counters(profile_id, followers, following);
    }

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
