// crates/profile/src/application/utils/fixture.rs

use std::sync::Arc;
// Shared Kernel
use shared_kernel::cache::CacheRepositoryStub;
use shared_kernel::command::CommandBus;
use shared_kernel::context::BaseAppContext;
use shared_kernel::core::Result;
use shared_kernel::idempotency::IdempotencyRepositoryStub;
use shared_kernel::messaging::OutboxRepositoryStub;
use shared_kernel::types::{AccountId, ProfileId, Region};

// Profile Domain & Application
use crate::application::context::{ProfileAppContext, ProfileContext};
use crate::commands::*;
use crate::entities::{Profile, ProfileBuilder};
use crate::repositories::ProfileRepositoryStub;
use crate::types::Handle;

pub struct ProfileTestFixture {
    bus: Arc<CommandBus>,
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
        let region = Region::default();
        let account_id = AccountId::generate(region);
        let profile_id = ProfileId::generate(region);
        let profile_ctx = ProfileContext::new(app_ctx.clone(), Some(profile_id), region);

        let mut bus = CommandBus::new();

        // --- Enregistrement des Handlers ---
        // AJOUT : Le bus principal doit connaître le handler de création pour les tests nominaux/retry
        bus.register::<ProfileContext, CreateProfileCommand, CreateProfileHandler>(
            CreateProfileHandler,
        );
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
            bus: Arc::new(bus),
            app_ctx,
            account_id,
            profile_ctx,
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
    pub fn profile_ctx(&self) -> &ProfileContext {
        &self.profile_ctx
    }

    // CORRECTION : L'accesseur .profile_id() renvoie un Result. On unwrap dans la fixture de test.
    pub fn profile_id(&self) -> ProfileId {
        self.profile_ctx
            .profile_id()
            .expect("L'ID du profil devrait être présent dans le contexte par défaut de la fixture")
            .clone()
    }

    pub fn region(&self) -> Region {
        self.profile_ctx.region()
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

        Profile::builder(self.account_id(), handle_vo)
            .expect("Erreur lors de l'initialisation du builder")
            .with_profile_id(self.profile_id())
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

    /// Clone la fixture courante mais applique un ProfileId et un Context différents.
    pub fn clone_with_profile_id(&self, new_profile_id: ProfileId) -> Self {
        let region = self.region();
        let profile_ctx = ProfileContext::new(self.app_ctx.clone(), Some(new_profile_id), region);
        let mut new_bus = CommandBus::new();

        new_bus.register::<ProfileContext, CreateProfileCommand, CreateProfileHandler>(
            CreateProfileHandler,
        );
        new_bus.register::<ProfileContext, UpdateDisplayNameCommand, UpdateDisplayNameHandler>(
            UpdateDisplayNameHandler,
        );
        new_bus.register::<ProfileContext, ChangeHandleCommand, ChangeHandleHandler>(
            ChangeHandleHandler,
        );

        Self {
            bus: Arc::new(new_bus),
            app_ctx: self.app_ctx.clone(),
            account_id: AccountId::generate(self.region()),
            profile_ctx,
            profile_repo: self.profile_repo.clone(),
            idempotency_repo: self.idempotency_repo.clone(),
            outbox_repo: self.outbox_repo.clone(),
        }
    }
}
