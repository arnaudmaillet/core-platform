// crates/profile/src/application/utils/fixture.rs

use std::sync::Arc;
// Shared Kernel
use shared_kernel::application::{BaseAppContext, CommandBus};
use shared_kernel::domain::repositories::{
    CacheRepositoryStub, IdempotencyRepositoryStub, OutboxRepositoryStub,
};
use shared_kernel::domain::value_objects::{AccountId, RegionCode};

// Profile Domain & Application
use crate::application::context::{ProfileAppContext, ProfileContext};
use crate::commands::*;
use crate::builders::ProfileBuilder;
use crate::entities::Profile;
use crate::repositories::ProfileRepositoryStub;
use crate::value_objects::{Handle, ProfileId};

pub struct ProfileTestFixture {
    bus: CommandBus,
    app_ctx: ProfileAppContext,
    account_id: AccountId,
    profile_ctx: ProfileContext,
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

        let base_ctx = BaseAppContext::new(None, cache);

        let app_ctx = ProfileAppContext::new(
            base_ctx,
            profile_repo.clone(),
            outbox_repo.clone(),
            idempotency_repo.clone(),
        );

        // Configuration par défaut pour les tests
        let region = RegionCode::default();
        let account_id = AccountId::generate(region.clone());
        let profile_id = ProfileId::generate();

        let profile_ctx = ProfileContext::new(app_ctx.clone(), profile_id, region);

        let mut bus = CommandBus::new();

        // --- Enregistrement des Handlers ---
        bus.register::<ProfileContext, UpdateDisplayNameCommand, UpdateDisplayNameHandler>(
            UpdateDisplayNameHandler,
        );
        bus.register::<ProfileContext, UpdateBioCommand, UpdateBioHandler>(UpdateBioHandler);
        bus.register::<ProfileContext, ChangeHandleCommand, ChangeHandleHandler>(
            ChangeHandleHandler,
        );
        bus.register::<ProfileContext, UpdatePrivacyCommand, UpdatePrivacyHandler>(
            UpdatePrivacyHandler,
        );
        bus.register::<ProfileContext, UpdateAvatarCommand, UpdateAvatarHandler>(
            UpdateAvatarHandler,
        );
        bus.register::<ProfileContext, RemoveAvatarCommand, RemoveAvatarHandler>(
            RemoveAvatarHandler,
        );
        bus.register::<ProfileContext, UpdateBannerCommand, UpdateBannerHandler>(
            UpdateBannerHandler,
        );
        bus.register::<ProfileContext, RemoveBannerCommand, RemoveBannerHandler>(
            RemoveBannerHandler,
        );
        bus.register::<ProfileContext, UpdateLocationCommand, UpdateLocationHandler>(
            UpdateLocationHandler,
        );
        bus.register::<ProfileContext, UpdateSocialsCommand, UpdateSocialsHandler>(
            UpdateSocialsHandler,
        );

        Self {
            bus,
            app_ctx,
            account_id,
            profile_ctx,
            profile_repo,
            idempotency_repo,
            outbox_repo,
        }
    }

    // --- Accesseurs ---

    pub fn bus(&self) -> &CommandBus {
        &self.bus
    }
    pub fn app_ctx(&self) -> &ProfileAppContext {
        &self.app_ctx
    }
    pub fn account_id(&self) -> AccountId {
        self.account_id.clone()
    }
    pub fn profile_ctx(&self) -> &ProfileContext {
        &self.profile_ctx
    }
    pub fn profile_id(&self) -> &ProfileId {
        self.profile_ctx.profile_id()
    }
    pub fn region(&self) -> RegionCode {
        self.profile_ctx.region().clone()
    }
    pub fn profile_repo(&self) -> &ProfileRepositoryStub {
        &self.profile_repo
    }

    pub fn idempotency_repo(&self) -> &IdempotencyRepositoryStub {
        &self.idempotency_repo
    }

    pub fn outbox_repo(&self) -> &OutboxRepositoryStub {
        &self.outbox_repo
    }

    // --- Helpers pour les tests ---

    /// Prépare un profil existant dans le repo pour le test
    pub async fn given_profile(&self, profile: Profile) {
        self.profile_repo.save_direct(profile).await;
    }

    pub fn builder(&self, handle: &str) -> ProfileBuilder {
        let handle_vo = Handle::try_new(handle).expect("Handle invalide dans la fixture");

        crate::domain::entities::Profile::builder(self.account_id(), handle_vo)
            .expect("Erreur lors de l'initialisation du builder")
            .with_profile_id(self.profile_id().clone()) // On force l'ID de la fixture
    }

    // --- Assertions ---

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

    pub async fn assert_profile<F>(&self, check: F)
    where
        F: FnOnce(&Profile),
    {
        let saved = self
            .profile_repo
            .find_direct(self.profile_id())
            .await
            .expect("Le profil devrait exister");
        check(&saved);
    }
}
