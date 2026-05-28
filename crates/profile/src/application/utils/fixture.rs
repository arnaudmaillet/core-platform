use shared_kernel::cache::CacheRepositoryStub;
use shared_kernel::command::CommandBus;
use shared_kernel::core::Result;
use shared_kernel::idempotency::IdempotencyRepositoryStub;
use shared_kernel::messaging::OutboxRepositoryStub;
use shared_kernel::types::{AccountId, ProfileId, Region};
use std::sync::Arc;

use crate::application::context::{ProfileAppContext, ProfileCommandContext, ProfileQueryContext};
use crate::commands::*;
use crate::entities::{Profile, ProfileBuilder};
use crate::repositories::ProfileRepositoryStub;
use crate::types::Handle;

pub struct ProfileTestFixture {
    bus: Arc<CommandBus>,
    region: Region,
    app_ctx: ProfileAppContext,
    account_id: AccountId,
    profile_id: ProfileId,
    command_ctx: ProfileCommandContext,
    query_ctx: ProfileQueryContext,
    profile_repo: Arc<ProfileRepositoryStub>,
    idempotency_repo: Arc<IdempotencyRepositoryStub>,
    outbox_repo: Arc<OutboxRepositoryStub>,
}

impl ProfileTestFixture {
    pub fn new() -> Self {
        let profile_repo = Arc::new(ProfileRepositoryStub::new());
        let outbox_repo = Arc::new(OutboxRepositoryStub::new());
        let idempotency_repo = Arc::new(IdempotencyRepositoryStub::new());
        let cache = Arc::new(CacheRepositoryStub::new());

        let app_ctx = ProfileAppContext::new_stubbed(
            profile_repo.clone(),
            outbox_repo.clone(),
            idempotency_repo.clone(),
        );

        let region = Region::default();
        let account_id = AccountId::generate();
        let profile_id = ProfileId::generate();

        let command_ctx = app_ctx.command(profile_id, region);
        let query_ctx = app_ctx.query(region);

        let mut bus = CommandBus::new(cache);

        bus.register::<ProfileCommandContext, CreateProfileCommand, CreateProfileHandler>(
            CreateProfileHandler,
        );
        bus.register::<ProfileCommandContext, UpdateDisplayNameCommand, UpdateDisplayNameHandler>(
            UpdateDisplayNameHandler,
        );
        bus.register::<ProfileCommandContext, UpdateBioCommand, UpdateBioHandler>(UpdateBioHandler);
        bus.register::<ProfileCommandContext, ChangeHandleCommand, ChangeHandleHandler>(
            ChangeHandleHandler,
        );
        bus.register::<ProfileCommandContext, UpdatePrivacyCommand, UpdatePrivacyHandler>(
            UpdatePrivacyHandler,
        );
        bus.register::<ProfileCommandContext, UpdateAvatarCommand, UpdateAvatarHandler>(
            UpdateAvatarHandler,
        );
        bus.register::<ProfileCommandContext, RemoveAvatarCommand, RemoveAvatarHandler>(
            RemoveAvatarHandler,
        );
        bus.register::<ProfileCommandContext, UpdateBannerCommand, UpdateBannerHandler>(
            UpdateBannerHandler,
        );
        bus.register::<ProfileCommandContext, RemoveBannerCommand, RemoveBannerHandler>(
            RemoveBannerHandler,
        );
        bus.register::<ProfileCommandContext, UpdateLocationCommand, UpdateLocationHandler>(
            UpdateLocationHandler,
        );
        bus.register::<ProfileCommandContext, UpdateSocialsCommand, UpdateSocialsHandler>(
            UpdateSocialsHandler,
        );

        Self {
            bus: Arc::new(bus),
            region,
            app_ctx,
            account_id,
            profile_id,
            command_ctx,
            query_ctx,
            profile_repo,
            idempotency_repo,
            outbox_repo,
        }
    }

    // --- Accesseurs ---

    pub fn bus(&self) -> Arc<CommandBus> {
        self.bus.clone()
    }

    pub fn app_ctx(&self) -> &ProfileAppContext {
        &self.app_ctx
    }

    pub fn account_id(&self) -> AccountId {
        self.account_id
    }

    pub fn command_ctx(&self) -> &ProfileCommandContext {
        &self.command_ctx
    }

    pub fn query_ctx(&self) -> &ProfileQueryContext {
        &self.query_ctx
    }

    pub fn profile_id(&self) -> ProfileId {
        self.profile_id
    }

    pub fn region(&self) -> Region {
        self.region
    }

    pub fn profile_repo(&self) -> &ProfileRepositoryStub {
        &self.profile_repo
    }

    pub fn idempotency_repo(&self) -> &IdempotencyRepositoryStub {
        &self.idempotency_repo
    }

    pub async fn given_profile(&self, profile: Profile) {
        self.profile_repo.save_direct(self.region, profile).await;
    }

    pub fn builder(&self, handle: &str) -> Result<ProfileBuilder> {
        let handle_vo = Handle::try_new(handle)?;

        Ok(
            Profile::builder(self.account_id(), self.profile_id(), handle_vo)?
                .with_profile_id(self.profile_id()),
        )
    }

    pub fn assert_outbox(&self, expected_count: usize, expected_event: Option<&str>) {
        assert_eq!(
            self.outbox_repo.count(),
            expected_count,
            "Nombre d'événements incorrect"
        );
        if let Some(name) = expected_event {
            assert!(
                self.outbox_repo.event_names().contains(&name.to_string()),
                "Événement {} manquant",
                name
            );
        }
    }

    pub async fn assert_profile<F>(&self, check: F) -> Result<()>
    where
        F: FnOnce(&Profile),
    {
        let saved_option = self.profile_repo.find_direct(self.profile_id()).await;

        let saved = saved_option.ok_or_else(|| {
            shared_kernel::core::Error::not_found("Profile", self.profile_id().to_string())
        })?;

        check(&saved);
        Ok(())
    }

    pub fn clone_with_profile_id(&self, new_profile_id: ProfileId) -> Self {
        let region: Region = self.region();
        let command_ctx = self.app_ctx.command(new_profile_id, region);
        let query_ctx = self.app_ctx.query(region);

        Self {
            bus: self.bus.clone(),
            region,
            app_ctx: self.app_ctx.clone(),
            account_id: AccountId::generate(),
            profile_id: new_profile_id,
            command_ctx,
            query_ctx,
            profile_repo: self.profile_repo.clone(),
            idempotency_repo: self.idempotency_repo.clone(),
            outbox_repo: self.outbox_repo.clone(),
        }
    }
}
