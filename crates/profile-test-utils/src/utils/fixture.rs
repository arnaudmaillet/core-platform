use profile::ProfileServiceBuilder;
use profile::repositories::ProfileRoutingRepository;
use shared_kernel::command::CommandBus;
use shared_kernel::core::Result;
use shared_kernel::types::{AccountId, ProfileId, Region};
use shared_kernel_test_utils::repositories::{CacheRepositoryStub, IdempotencyRepositoryStub};
use std::sync::Arc;

use crate::repositories::{ProfileRepositoryStub, ProfileRoutingRepositoryStub};
use profile::context::{ProfileAppContext, ProfileCommandContext, ProfileQueryContext};
use profile::entities::{Profile, ProfileBuilder};
use profile::types::Handle;

pub struct ProfileTestFixture {
    bus: Arc<CommandBus>,
    app_ctx: ProfileAppContext,
    account_id: AccountId,
    profile_id: ProfileId,
    query_ctx: ProfileQueryContext,
    profile_repo: Arc<ProfileRepositoryStub>,
    routing_repo: Arc<ProfileRoutingRepositoryStub>,
    idempotency_repo: Arc<IdempotencyRepositoryStub>,
}

impl ProfileTestFixture {
    pub fn new() -> Self {
        let profile_repo = Arc::new(ProfileRepositoryStub::new());
        let routing_repo = Arc::new(ProfileRoutingRepositoryStub::new());
        let cache_repo = Arc::new(CacheRepositoryStub::new());
        let idempotency_repo = Arc::new(IdempotencyRepositoryStub::new());
        let region = Region::default();

        let builder = ProfileServiceBuilder::new(
            profile_repo.clone(),
            routing_repo.clone(),
            cache_repo.clone(),
            idempotency_repo.clone(),
            region,
        );
        
        let app_ctx = builder.build_context();
        let bus = builder.build_command_bus();

        let account_id = AccountId::generate();
        let profile_id = ProfileId::generate();
        let query_ctx = app_ctx.query();

        Self {
            bus,
            app_ctx,
            account_id,
            profile_id,
            query_ctx,
            profile_repo,
            routing_repo,
            idempotency_repo,
        }
    }

    // --- Configuration des stubs de données (Arrange) ---

    pub async fn given_profile(&self, profile: Profile) {
        self.profile_repo.save_direct(profile).await;
    }

    pub async fn given_slug_routing(&self, profile_id: ProfileId, slug_hash: &str, region: Region) {
        self.routing_repo
            .register_routing(profile_id, slug_hash, region)
            .await
            .expect("Failed to setup slug routing in test fixture");
    }

    pub fn builder(&self, handle: &str) -> Result<ProfileBuilder> {
        let handle_vo = Handle::try_new(handle)?;
        Ok(
            Profile::builder(self.account_id(), self.profile_id(), handle_vo)?
                .with_profile_id(self.profile_id()),
        )
    }

    // --- Accesseurs d'infrastructure explicites ---

    pub fn bus(&self) -> Arc<CommandBus> {
        self.bus.clone()
    }
    pub fn region(&self) -> Region {
        self.app_ctx.local_region()
    }
    pub fn account_id(&self) -> AccountId {
        self.account_id
    }
    pub fn profile_id(&self) -> ProfileId {
        self.profile_id
    }

    pub fn profile_repo(&self) -> &ProfileRepositoryStub {
        &self.profile_repo
    }
    pub fn routing_repo(&self) -> &ProfileRoutingRepositoryStub {
        &self.routing_repo
    }
    pub fn idempotency_repo(&self) -> &IdempotencyRepositoryStub {
        &self.idempotency_repo
    }

    // --- Context Factories Explicites ---

    pub fn command_ctx(&self) -> ProfileCommandContext {
        self.app_ctx.command(self.profile_id)
    }
    pub fn command_ctx_for(&self, id: ProfileId) -> ProfileCommandContext {
        self.app_ctx.command(id)
    }
    pub fn creation_ctx(&self) -> ProfileCommandContext {
        self.app_ctx.creation_command()
    }
}
