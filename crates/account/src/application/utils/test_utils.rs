// crates/account/src/application/test_utils.rs

use std::sync::Arc;

// Shared Kernel
use shared_kernel::application::{BaseAppContext, CommandBus};
use shared_kernel::domain::repositories::{
    CacheRepositoryStub, IdempotencyRepositoryStub, OutboxRepositoryStub,
};
use shared_kernel::domain::value_objects::{AccountId, RegionCode};
use shared_kernel::core::Result;

// Account Domain & Application
// Note : Importation directe depuis application::context (structure plate)
use crate::application::context::{AccountAppContext, AccountContext};
use crate::domain::account::builders::AccountBuilder;
use crate::domain::account::entities::Account;
use crate::domain::repositories::AccountRepositoryStub;
use crate::domain::value_objects::RegistrationIdentifier;
use crate::use_cases::lifecycle::change_beta_tier::change_beta_tier_use_case::ChangeBetaTierHandler;
use crate::use_cases::{
    ActivateCommand, ActivateHandler, AddPushTokenCommand, AddPushTokenHandler, BanCommand,
    BanHandler, ChangeBetaTierCommand, ChangeBirthDateCommand, ChangeBirthDateHandler,
    ChangeEmailCommand, ChangeEmailHandler, ChangePhoneNumberCommand, ChangePhoneNumberHandler,
    ChangeRegionCommand, ChangeRegionHandler, ChangeRoleCommand, ChangeRoleHandler,
    DeactivateCommand, DeactivateHandler, DecreaseTrustScoreCommand, DecreaseTrustScoreHandler,
    IncreaseTrustScoreCommand, IncreaseTrustScoreHandler, LiftShadowbanCommand,
    LiftShadowbanHandler, LinkSubIdentityCommand, LinkSubIdentityHandler, RegisterCommand,
    RegisterHandler, RemovePushTokenCommand, RemovePushTokenHandler, ShadowbanCommand,
    ShadowbanHandler, SuspendCommand, SuspendHandler, UnbanCommand, UnbanHandler, UnsuspendCommand,
    UnsuspendHandler, UpdateLocaleCommand, UpdateLocaleHandler, UpdatePreferencesCommand,
    UpdatePreferencesHandler, UpdateTimezoneCommand, UpdateTimezoneHandler,
};

// --- Imports des Use Cases ---

pub struct TestFixture {
    bus: CommandBus,
    app_ctx: AccountAppContext,
    account_ctx: AccountContext,
    account_repo: Arc<AccountRepositoryStub>,
    idempotency_repo: Arc<IdempotencyRepositoryStub>,
    outbox_repo: Arc<OutboxRepositoryStub>,
}

impl TestFixture {
    pub fn new() -> Self {
        let account_repo = Arc::new(AccountRepositoryStub::new());
        let outbox_repo = Arc::new(OutboxRepositoryStub::new());
        let idempotency_repo = Arc::new(IdempotencyRepositoryStub::new());
        let cache = Arc::new(CacheRepositoryStub::new());

        let base_ctx = BaseAppContext::new(None, cache);

        let app_ctx = AccountAppContext::new(
            base_ctx,
            account_repo.clone(),
            outbox_repo.clone(),
            idempotency_repo.clone(),
        );

        let account_id = AccountId::generate(RegionCode::default());
        let account_ctx = AccountContext::new(app_ctx.clone(), account_id);

        let mut bus = CommandBus::new();

        // Enregistrement des Handlers avec le type AccountContext propre
        bus.register::<AccountContext, RegisterCommand, RegisterHandler>(RegisterHandler);
        bus.register::<AccountContext, LinkSubIdentityCommand, LinkSubIdentityHandler>(
            LinkSubIdentityHandler,
        );
        bus.register::<AccountContext, ActivateCommand, ActivateHandler>(ActivateHandler);
        bus.register::<AccountContext, DeactivateCommand, DeactivateHandler>(DeactivateHandler);
        bus.register::<AccountContext, ChangeRoleCommand, ChangeRoleHandler>(ChangeRoleHandler);
        bus.register::<AccountContext, ChangeBetaTierCommand, ChangeBetaTierHandler>(
            ChangeBetaTierHandler,
        );
        bus.register::<AccountContext, SuspendCommand, SuspendHandler>(SuspendHandler);
        bus.register::<AccountContext, UnsuspendCommand, UnsuspendHandler>(UnsuspendHandler);
        bus.register::<AccountContext, BanCommand, BanHandler>(BanHandler);
        bus.register::<AccountContext, UnbanCommand, UnbanHandler>(UnbanHandler);
        bus.register::<AccountContext, ShadowbanCommand, ShadowbanHandler>(ShadowbanHandler);
        bus.register::<AccountContext, LiftShadowbanCommand, LiftShadowbanHandler>(
            LiftShadowbanHandler,
        );
        bus.register::<AccountContext, IncreaseTrustScoreCommand, IncreaseTrustScoreHandler>(
            IncreaseTrustScoreHandler,
        );
        bus.register::<AccountContext, DecreaseTrustScoreCommand, DecreaseTrustScoreHandler>(
            DecreaseTrustScoreHandler,
        );
        bus.register::<AccountContext, AddPushTokenCommand, AddPushTokenHandler>(
            AddPushTokenHandler,
        );
        bus.register::<AccountContext, RemovePushTokenCommand, RemovePushTokenHandler>(
            RemovePushTokenHandler,
        );
        bus.register::<AccountContext, ChangeEmailCommand, ChangeEmailHandler>(ChangeEmailHandler);
        bus.register::<AccountContext, ChangePhoneNumberCommand, ChangePhoneNumberHandler>(
            ChangePhoneNumberHandler,
        );
        bus.register::<AccountContext, ChangeBirthDateCommand, ChangeBirthDateHandler>(
            ChangeBirthDateHandler,
        );
        bus.register::<AccountContext, ChangeRegionCommand, ChangeRegionHandler>(
            ChangeRegionHandler,
        );
        bus.register::<AccountContext, UpdateLocaleCommand, UpdateLocaleHandler>(
            UpdateLocaleHandler,
        );
        bus.register::<AccountContext, UpdateTimezoneCommand, UpdateTimezoneHandler>(
            UpdateTimezoneHandler,
        );
        bus.register::<AccountContext, UpdatePreferencesCommand, UpdatePreferencesHandler>(
            UpdatePreferencesHandler,
        );

        Self {
            bus,
            app_ctx,
            account_ctx,
            account_repo,
            idempotency_repo,
            outbox_repo,
        }
    }

    // --- Accesseurs ---

    pub fn bus(&self) -> &CommandBus {
        &self.bus
    }

    pub fn app_ctx(&self) -> &AccountAppContext {
        &self.app_ctx
    }

    pub fn account_ctx(&self) -> &AccountContext {
        &self.account_ctx
    }

    pub fn account_id(&self) -> AccountId {
        self.account_ctx.account_id().clone()
    }

    pub fn region(&self) -> RegionCode {
        self.account_ctx.region().clone()
    }

    pub fn account_repo(&self) -> &AccountRepositoryStub {
        &self.account_repo
    }

    pub fn idempotency_repo(&self) -> &IdempotencyRepositoryStub {
        &self.idempotency_repo
    }

    pub fn outbox_repo(&self) -> &OutboxRepositoryStub {
        &self.outbox_repo
    }

    pub fn outbox_events(&self) -> Vec<String> {
        self.outbox_repo.event_names()
    }

    pub fn account_builder(&self) -> Result<AccountBuilder> {
        self.account_builder_for(self.region())
    }

    pub fn account_builder_for(&self, region: RegionCode) -> Result<AccountBuilder> {
        Ok(Account::builder(
            self.account_id(),
            RegistrationIdentifier::try_from_email("test@example.com")?,
        ))
    }

    // --- Assertions ---

    pub fn assert_outbox(&self, expected_count: usize, expected_event: Option<&str>) {
        assert_eq!(
            self.outbox_repo().count(),
            expected_count,
            "Nombre d'événements incorrect"
        );
        if let Some(event_name) = expected_event {
            assert!(
                self.outbox_events().contains(&event_name.to_string()),
                "L'événement {} est manquant dans l'outbox",
                event_name
            );
        }
    }

    pub async fn assert_account<F>(&self, check: F) -> Result<()>
    where
        F: FnOnce(&Account),
    {
        self.assert_account_by_id(&self.account_id(), check).await
    }

    pub async fn assert_account_by_id<F>(&self, id: &AccountId, check: F) -> Result<()>
    where
        F: FnOnce(&Account),
    {
        let saved = self
            .account_repo
            .find_direct(id)
            .expect("Le compte devrait exister dans le repository");
        check(&saved);
        Ok(())
    }

    pub async fn assert_account_exists(&self, id: &AccountId) -> Result<()> {
        assert!(self.account_repo().find_direct(id).is_some());
        Ok(())
    }

    pub fn assert_outbox_contains(&self, event_name: &str) {
        assert!(
            self.outbox_events().contains(&event_name.to_string()),
            "L'événement {} est manquant. Présents : {:?}",
            event_name,
            self.outbox_events()
        );
    }
}
