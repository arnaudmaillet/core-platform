// crates/account/src/application/fixture.rs

use crate::repositories::{AccountRepositoryStub, GlobalIdentityRegistryStub, OtpRepositoryStub};
use account::commands::{
    ActivateCommand, ActivateHandler, AddPushTokenCommand, AddPushTokenHandler, BanCommand,
    BanHandler, ChangeBetaTierCommand, ChangeBetaTierHandler, ChangeBirthDateCommand,
    ChangeBirthDateHandler, ChangeEmailCommand, ChangeEmailHandler, ChangePhoneCommand,
    ChangePhoneNumberHandler, ChangeRoleCommand, ChangeRoleHandler, DeactivateCommand,
    DeactivateHandler, DecreaseTrustScoreCommand, DecreaseTrustScoreHandler,
    IncreaseTrustScoreCommand, IncreaseTrustScoreHandler, LiftShadowbanCommand,
    LiftShadowbanHandler, LinkSubIdentityCommand, LinkSubIdentityHandler, RegisterCommand,
    RegisterHandler, RemovePushTokenCommand, RemovePushTokenHandler, ShadowbanCommand,
    ShadowbanHandler, SuspendCommand, SuspendHandler, UnbanCommand, UnbanHandler, UnsuspendCommand,
    UnsuspendHandler, UpdateLocaleCommand, UpdateLocaleHandler, UpdatePreferencesCommand,
    UpdatePreferencesHandler, UpdateTimezoneCommand, UpdateTimezoneHandler, VerifyEmailCommand,
    VerifyEmailHandler, VerifyPhoneCommand, VerifyPhoneHandler,
};
use account::context::{AccountAppContext, AccountCommandContext, AccountQueryContext};
use account::entities::{Account, AccountBuilder};
use account::types::RegistrationIdentifier;
use shared_kernel::command::CommandBus;
use shared_kernel::core::Result;
use shared_kernel::types::{AccountId, Email, Region};
use shared_kernel_test_utils::repositories::IdempotencyRepositoryStub;
use shared_kernel_test_utils::repositories::OutboxRepositoryStub;
use shared_kernel_test_utils::repositories::{CacheRepositoryStub, TransactionManagerStub};
use std::sync::Arc;

// --- Imports des Use Cases ---

pub struct AccountTestFixture {
    bus: CommandBus,
    region: Region,
    account_id: AccountId,
    app_ctx: AccountAppContext<TransactionManagerStub>,
    command_ctx: AccountCommandContext<TransactionManagerStub>,
    query_ctx: AccountQueryContext<TransactionManagerStub>,
    account_repo: Arc<AccountRepositoryStub>,
    idempotency_repo: Arc<IdempotencyRepositoryStub>,
    outbox_repo: Arc<OutboxRepositoryStub>,
    global_registry: Arc<GlobalIdentityRegistryStub>,
    otp_repo: Arc<OtpRepositoryStub>,
}

impl AccountTestFixture {
    pub fn new() -> Self {
        let tx_manager = Arc::new(TransactionManagerStub);
        let account_repo = Arc::new(AccountRepositoryStub::new());
        let outbox_repo = Arc::new(OutboxRepositoryStub::new());
        let idempotency_repo = Arc::new(IdempotencyRepositoryStub::new());
        let cache = Arc::new(CacheRepositoryStub::new());
        let global_registry = Arc::new(GlobalIdentityRegistryStub::new());
        let otp_repo = Arc::new(OtpRepositoryStub::new());

        let app_ctx = AccountAppContext::new(
            tx_manager,
            account_repo.clone(),
            outbox_repo.clone(),
            idempotency_repo.clone(),
            global_registry.clone(),
        );

        let region = Region::default();
        let account_id = AccountId::generate();
        let command_ctx = app_ctx.command(account_id, region);
        let query_ctx = app_ctx.query(region);

        let mut bus = CommandBus::new(cache);

        bus.register::<AccountCommandContext<TransactionManagerStub>, RegisterCommand, RegisterHandler<TransactionManagerStub>>(RegisterHandler::new());
        bus.register::<AccountCommandContext<TransactionManagerStub>, LinkSubIdentityCommand, LinkSubIdentityHandler<TransactionManagerStub>>(
            LinkSubIdentityHandler::new(),
        );
        bus.register::<AccountCommandContext<TransactionManagerStub>, VerifyEmailCommand, VerifyEmailHandler<TransactionManagerStub>>(
            VerifyEmailHandler::new(otp_repo.clone()),
        );
        bus.register::<AccountCommandContext<TransactionManagerStub>, VerifyPhoneCommand, VerifyPhoneHandler<TransactionManagerStub>>(
            VerifyPhoneHandler::new(otp_repo.clone()),
        );
        bus.register::<AccountCommandContext<TransactionManagerStub>, ActivateCommand, ActivateHandler<TransactionManagerStub>>(ActivateHandler::new());
        bus.register::<AccountCommandContext<TransactionManagerStub>, DeactivateCommand, DeactivateHandler<TransactionManagerStub>>(
            DeactivateHandler::new(),
        );
        bus.register::<AccountCommandContext<TransactionManagerStub>, ChangeRoleCommand, ChangeRoleHandler<TransactionManagerStub>>(
            ChangeRoleHandler::new(),
        );
        bus.register::<AccountCommandContext<TransactionManagerStub>, ChangeBetaTierCommand, ChangeBetaTierHandler<TransactionManagerStub>>(
            ChangeBetaTierHandler::new(),
        );
        bus.register::<AccountCommandContext<TransactionManagerStub>, SuspendCommand, SuspendHandler<TransactionManagerStub>>(SuspendHandler::new());
        bus.register::<AccountCommandContext<TransactionManagerStub>, UnsuspendCommand, UnsuspendHandler<TransactionManagerStub>>(UnsuspendHandler::new());
        bus.register::<AccountCommandContext<TransactionManagerStub>, BanCommand, BanHandler<TransactionManagerStub>>(BanHandler::new());
        bus.register::<AccountCommandContext<TransactionManagerStub>, UnbanCommand, UnbanHandler<TransactionManagerStub>>(UnbanHandler::new());
        bus.register::<AccountCommandContext<TransactionManagerStub>, ShadowbanCommand, ShadowbanHandler<TransactionManagerStub>>(ShadowbanHandler::new());
        bus.register::<AccountCommandContext<TransactionManagerStub>, LiftShadowbanCommand, LiftShadowbanHandler<TransactionManagerStub>>(
            LiftShadowbanHandler::new(),
        );
        bus.register::<AccountCommandContext<TransactionManagerStub>, IncreaseTrustScoreCommand, IncreaseTrustScoreHandler<TransactionManagerStub>>(
            IncreaseTrustScoreHandler::new(),
        );
        bus.register::<AccountCommandContext<TransactionManagerStub>, DecreaseTrustScoreCommand, DecreaseTrustScoreHandler<TransactionManagerStub>>(
            DecreaseTrustScoreHandler::new(),
        );
        bus.register::<AccountCommandContext<TransactionManagerStub>, AddPushTokenCommand, AddPushTokenHandler<TransactionManagerStub>>(
            AddPushTokenHandler::new(),
        );
        bus.register::<AccountCommandContext<TransactionManagerStub>, RemovePushTokenCommand, RemovePushTokenHandler<TransactionManagerStub>>(
            RemovePushTokenHandler::new(),
        );
        bus.register::<AccountCommandContext<TransactionManagerStub>, ChangeEmailCommand, ChangeEmailHandler<TransactionManagerStub>>(
            ChangeEmailHandler::new(),
        );
        bus.register::<AccountCommandContext<TransactionManagerStub>, ChangePhoneCommand, ChangePhoneNumberHandler<TransactionManagerStub>>(
            ChangePhoneNumberHandler::new(),
        );
        bus.register::<AccountCommandContext<TransactionManagerStub>, ChangeBirthDateCommand, ChangeBirthDateHandler<TransactionManagerStub>>(
            ChangeBirthDateHandler::new(),
        );
        bus.register::<AccountCommandContext<TransactionManagerStub>, UpdateLocaleCommand, UpdateLocaleHandler<TransactionManagerStub>>(
            UpdateLocaleHandler::new(),
        );
        bus.register::<AccountCommandContext<TransactionManagerStub>, UpdateTimezoneCommand, UpdateTimezoneHandler<TransactionManagerStub>>(
            UpdateTimezoneHandler::new(),
        );
        bus.register::<AccountCommandContext<TransactionManagerStub>, UpdatePreferencesCommand, UpdatePreferencesHandler<TransactionManagerStub>>(
            UpdatePreferencesHandler::new(),
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
            global_registry,
            otp_repo,
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

    pub fn app_ctx(&self) -> &AccountAppContext<TransactionManagerStub> {
        &self.app_ctx
    }

    pub fn command_ctx(&self) -> &AccountCommandContext<TransactionManagerStub> {
        &self.command_ctx
    }

    pub fn query_ctx(&self) -> &AccountQueryContext<TransactionManagerStub> {
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

    pub fn global_registry(&self) -> &GlobalIdentityRegistryStub {
        &self.global_registry
    }

    pub fn otp_repo(&self) -> Arc<OtpRepositoryStub> {
        self.otp_repo.clone()
    }

    pub fn builder(&self) -> Result<AccountBuilder> {
        Ok(Account::builder(
            self.account_id,
            RegistrationIdentifier::from_email(Email::try_new("test@example.com")?),
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
