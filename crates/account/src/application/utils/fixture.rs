// crates/account/src/application/fixture.rs

use std::sync::Arc;

// Shared Kernel
use crate::application::context::{AccountAppContext, AccountCommandContext, AccountQueryContext};
use crate::commands::lifecycle::change_beta_tier::change_beta_tier_handler::ChangeBetaTierHandler;
use crate::commands::{
    ActivateCommand, ActivateHandler, AddPushTokenCommand, AddPushTokenHandler, BanCommand,
    BanHandler, ChangeBetaTierCommand, ChangeBirthDateCommand, ChangeBirthDateHandler,
    ChangeEmailCommand, ChangeEmailHandler, ChangePhoneNumberCommand, ChangePhoneNumberHandler,
    ChangeRoleCommand, ChangeRoleHandler, DeactivateCommand, DeactivateHandler,
    DecreaseTrustScoreCommand, DecreaseTrustScoreHandler, IncreaseTrustScoreCommand,
    IncreaseTrustScoreHandler, LiftShadowbanCommand, LiftShadowbanHandler, LinkSubIdentityCommand,
    LinkSubIdentityHandler, RegisterCommand, RegisterHandler, RemovePushTokenCommand,
    RemovePushTokenHandler, ShadowbanCommand, ShadowbanHandler, SuspendCommand, SuspendHandler,
    UnbanCommand, UnbanHandler, UnsuspendCommand, UnsuspendHandler, UpdateLocaleCommand,
    UpdateLocaleHandler, UpdatePreferencesCommand, UpdatePreferencesHandler, UpdateTimezoneCommand,
    UpdateTimezoneHandler,
};
use crate::domain::repositories::AccountRepositoryStub;
use crate::domain::types::RegistrationIdentifier;
use crate::entities::{Account, AccountBuilder};
use shared_kernel::cache::CacheRepositoryStub;
use shared_kernel::command::CommandBus;
use shared_kernel::core::Result;
use shared_kernel::idempotency::IdempotencyRepositoryStub;
use shared_kernel::messaging::OutboxRepositoryStub;
use shared_kernel::types::{AccountId, Region};

// --- Imports des Use Cases ---

pub struct TestFixture {
    bus: CommandBus,
    region: Region,
    account_id: AccountId,
    app_ctx: AccountAppContext,
    command_ctx: AccountCommandContext,
    query_ctx: AccountQueryContext,
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

        let app_ctx = AccountAppContext::new_stubbed(
            account_repo.clone(),
            outbox_repo.clone(),
            idempotency_repo.clone(),
        );

        // Configuration par défaut pour les tests
        let region = Region::default();
        let account_id = AccountId::generate();
        let command_ctx = app_ctx.command(account_id, region);
        let query_ctx = app_ctx.query(region);

        let mut bus = CommandBus::new(cache);

        bus.register::<AccountCommandContext, RegisterCommand, RegisterHandler>(RegisterHandler);
        bus.register::<AccountCommandContext, LinkSubIdentityCommand, LinkSubIdentityHandler>(
            LinkSubIdentityHandler,
        );
        bus.register::<AccountCommandContext, ActivateCommand, ActivateHandler>(ActivateHandler);
        bus.register::<AccountCommandContext, DeactivateCommand, DeactivateHandler>(
            DeactivateHandler,
        );
        bus.register::<AccountCommandContext, ChangeRoleCommand, ChangeRoleHandler>(
            ChangeRoleHandler,
        );
        bus.register::<AccountCommandContext, ChangeBetaTierCommand, ChangeBetaTierHandler>(
            ChangeBetaTierHandler,
        );
        bus.register::<AccountCommandContext, SuspendCommand, SuspendHandler>(SuspendHandler);
        bus.register::<AccountCommandContext, UnsuspendCommand, UnsuspendHandler>(UnsuspendHandler);
        bus.register::<AccountCommandContext, BanCommand, BanHandler>(BanHandler);
        bus.register::<AccountCommandContext, UnbanCommand, UnbanHandler>(UnbanHandler);
        bus.register::<AccountCommandContext, ShadowbanCommand, ShadowbanHandler>(ShadowbanHandler);
        bus.register::<AccountCommandContext, LiftShadowbanCommand, LiftShadowbanHandler>(
            LiftShadowbanHandler,
        );
        bus.register::<AccountCommandContext, IncreaseTrustScoreCommand, IncreaseTrustScoreHandler>(
            IncreaseTrustScoreHandler,
        );
        bus.register::<AccountCommandContext, DecreaseTrustScoreCommand, DecreaseTrustScoreHandler>(
            DecreaseTrustScoreHandler,
        );
        bus.register::<AccountCommandContext, AddPushTokenCommand, AddPushTokenHandler>(
            AddPushTokenHandler,
        );
        bus.register::<AccountCommandContext, RemovePushTokenCommand, RemovePushTokenHandler>(
            RemovePushTokenHandler,
        );
        bus.register::<AccountCommandContext, ChangeEmailCommand, ChangeEmailHandler>(
            ChangeEmailHandler,
        );
        bus.register::<AccountCommandContext, ChangePhoneNumberCommand, ChangePhoneNumberHandler>(
            ChangePhoneNumberHandler,
        );
        bus.register::<AccountCommandContext, ChangeBirthDateCommand, ChangeBirthDateHandler>(
            ChangeBirthDateHandler,
        );
        bus.register::<AccountCommandContext, UpdateLocaleCommand, UpdateLocaleHandler>(
            UpdateLocaleHandler,
        );
        bus.register::<AccountCommandContext, UpdateTimezoneCommand, UpdateTimezoneHandler>(
            UpdateTimezoneHandler,
        );
        bus.register::<AccountCommandContext, UpdatePreferencesCommand, UpdatePreferencesHandler>(
            UpdatePreferencesHandler,
        );

        Self {
            bus,
            region,
            account_id,
            app_ctx: app_ctx,
            command_ctx,
            query_ctx,
            account_repo,
            idempotency_repo,
            outbox_repo,
        }
    }

    // --- Accesseurs ---

    pub fn bus(&self) -> &CommandBus {
        &self.bus
    }

    pub fn account_id(&self) -> AccountId {
        self.account_id
    }

    pub fn region(&self) -> Region {
        self.region
    }

    pub fn app_ctx(&self) -> &AccountAppContext {
        &self.app_ctx
    }

    pub fn command_ctx(&self) -> &AccountCommandContext {
        &self.command_ctx
    }

    pub fn query_ctx(&self) -> &AccountQueryContext {
        &self.query_ctx
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

    pub fn builder(&self) -> Result<AccountBuilder> {
        Ok(Account::builder(
            self.account_id,
            RegistrationIdentifier::try_from_email("test@example.com")?,
        ))
    }

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
        self.assert_account_by_id(self.account_id, check).await
    }

    pub async fn assert_account_by_id<F>(&self, id: AccountId, check: F) -> Result<()>
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

    pub async fn assert_account_exists(&self, id: AccountId) -> Result<()> {
        assert!(self.account_repo().find_direct(id).is_some());
        Ok(())
    }
}
