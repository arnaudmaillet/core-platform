use profile::ProfileServiceBuilder;
use profile::repositories::ProfileRoutingRepository;
use shared_kernel::command::CommandBus;
use shared_kernel::core::Result;
use shared_kernel::environment::ClusterContext;
use shared_kernel::types::{AccountId, ProfileId, Region};
use shared_kernel_test_utils::repositories::{CacheRepositoryStub, IdempotencyRepositoryStub};
use std::sync::Arc;

use crate::repositories::{ProfileRepositoryStub, ProfileRoutingRepositoryStub};
use profile::context::{ProfileCommandCtx, ProfileKernelCtx, ProfileQueryCtx};
use profile::entities::{Profile, ProfileBuilder};
use profile::types::Handle;

pub struct ProfileTestFixture {
    bus: CommandBus,

    kernel_ctx: ProfileKernelCtx,
    command_ctx: ProfileCommandCtx,
    query_ctx: ProfileQueryCtx,
    cluster_ctx: ClusterContext,

    account_id: AccountId,
    profile_id: ProfileId,
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

        let cluster_ctx = ClusterContext::default();
        let profile_id = ProfileId::generate();
        let account_id = AccountId::generate();

        let service = ProfileServiceBuilder::new(
            profile_repo.clone(),
            routing_repo.clone(),
            idempotency_repo.clone(),
            cluster_ctx,
        );

        let kernel_ctx = service.build_kernel_ctx();
        let command_ctx =
            ProfileCommandCtx::new(kernel_ctx.clone(), Some(profile_id), cluster_ctx.region());
        let query_ctx = ProfileQueryCtx::new(kernel_ctx.clone());

        let mut bus = CommandBus::new(Some(idempotency_repo.clone()), Some(cache_repo));
        service.register_handlers(&mut bus);

        Self {
            bus,
            kernel_ctx,
            command_ctx,
            query_ctx,
            cluster_ctx,
            account_id,
            profile_id,
            profile_repo,
            routing_repo,
            idempotency_repo,
        }
    }

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

    pub fn bus(&self) -> &CommandBus {
        &self.bus
    }
    pub fn region(&self) -> Region {
        self.kernel_ctx.server_region()
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
    pub fn command_ctx(&self) -> &ProfileCommandCtx {
        &self.command_ctx
    }
    pub fn creation_ctx(&self, region: Region) -> ProfileCommandCtx {
        self.kernel_ctx.creation_command(region)
    }
}
