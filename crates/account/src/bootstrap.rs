use std::{sync::Arc, time::Duration};

use crate::{
    commands::{
        ActivateCommand, ActivateHandler, AddPushTokenCommand, AddPushTokenHandler, BanCommand,
        BanHandler, ChangeBirthDateCommand, ChangeBirthDateHandler, ChangeEmailCommand,
        ChangeEmailHandler, ChangePhoneCommand, ChangePhoneNumberHandler, ChangeRoleCommand,
        ChangeRoleHandler, DeactivateCommand, DeactivateHandler, DecreaseTrustScoreCommand,
        DecreaseTrustScoreHandler, IncreaseTrustScoreCommand, IncreaseTrustScoreHandler,
        LiftShadowbanCommand, LiftShadowbanHandler, LinkSubIdentityCommand, LinkSubIdentityHandler,
        RegisterCommand, RegisterHandler, RemovePushTokenCommand, RemovePushTokenHandler,
        ShadowbanCommand, ShadowbanHandler, SuspendCommand, SuspendHandler, UnbanCommand,
        UnbanHandler, UnsuspendCommand, UnsuspendHandler, UpdateLocaleCommand, UpdateLocaleHandler,
        UpdatePreferencesCommand, UpdatePreferencesHandler, UpdateTimezoneCommand,
        UpdateTimezoneHandler, VerifyEmailCommand, VerifyEmailHandler, VerifyPhoneCommand,
        VerifyPhoneHandler,
    },
    context::{AccountAppContext, AccountCommandContext},
    db::{PostgresAccountRepository, PostgresGlobalIdentityRegistry},
    fred::FredOtpRepository,
    repositories::OtpRepository,
};
use infra_sqlx::{
    PostgresIdempotencyRepository, PostgresOutboxRepository, PostgresTransactionManager,
    sqlx::PgPool,
};
use shared_kernel::{cache::CacheRepository, command::CommandBus};

pub struct AccountServiceBuilder {
    pool: PgPool,
    global_pool: PgPool,
    cache_repo: Arc<dyn CacheRepository>,
    otp_ttl: Duration,
}

impl AccountServiceBuilder {
    pub fn new(
        pool: PgPool,
        global_pool: PgPool,
        cache_repo: Arc<dyn CacheRepository>,
        otp_ttl: Duration,
    ) -> Self {
        Self {
            pool,
            global_pool,
            cache_repo,
            otp_ttl,
        }
    }

    pub fn build_context(&self) -> Arc<AccountAppContext<PostgresTransactionManager>> {
        let tx_manager = Arc::new(PostgresTransactionManager::new(self.pool.clone()));
        let account_repo = Arc::new(PostgresAccountRepository::new(self.pool.clone()));
        let outbox_repo = Arc::new(PostgresOutboxRepository::new(self.pool.clone()));
        let idempotency_repo = Arc::new(PostgresIdempotencyRepository::new("account_idempotency"));
        let global_registry = Arc::new(PostgresGlobalIdentityRegistry::new(
            self.global_pool.clone(),
        ));

        Arc::new(AccountAppContext::new(
            tx_manager,
            account_repo,
            outbox_repo,
            idempotency_repo,
            global_registry,
        ))
    }

    pub fn build_command_bus(&self) -> Arc<CommandBus> {
        let mut bus = CommandBus::new(self.cache_repo.clone());
        let otp_repo: Arc<dyn OtpRepository> = Arc::new(FredOtpRepository::new(
            self.cache_repo.clone(),
            self.otp_ttl,
        ));

        bus.register::<AccountCommandContext<PostgresTransactionManager>, RegisterCommand, RegisterHandler<PostgresTransactionManager>>(RegisterHandler::new());
        bus.register::<AccountCommandContext<PostgresTransactionManager>, LinkSubIdentityCommand, LinkSubIdentityHandler<PostgresTransactionManager>>(
            LinkSubIdentityHandler::new(),
        );
        bus.register::<AccountCommandContext<PostgresTransactionManager>, RegisterCommand, RegisterHandler<PostgresTransactionManager>>(RegisterHandler::new());
        bus.register::<AccountCommandContext<PostgresTransactionManager>, LinkSubIdentityCommand, LinkSubIdentityHandler<PostgresTransactionManager>>(
            LinkSubIdentityHandler::new(),
        );
        bus.register::<AccountCommandContext<PostgresTransactionManager>, VerifyEmailCommand, VerifyEmailHandler<PostgresTransactionManager>>(
            VerifyEmailHandler::new(otp_repo.clone()),
        );
        bus.register::<AccountCommandContext<PostgresTransactionManager>, VerifyPhoneCommand, VerifyPhoneHandler<PostgresTransactionManager>>(
            VerifyPhoneHandler::new(otp_repo),
        );
        bus.register::<AccountCommandContext<PostgresTransactionManager>, ActivateCommand, ActivateHandler<PostgresTransactionManager>>(ActivateHandler::new());
        bus.register::<AccountCommandContext<PostgresTransactionManager>, DeactivateCommand, DeactivateHandler<PostgresTransactionManager>>(
            DeactivateHandler::new(),
        );
        bus.register::<AccountCommandContext<PostgresTransactionManager>, ChangeRoleCommand, ChangeRoleHandler<PostgresTransactionManager>>(
            ChangeRoleHandler::new(),
        );
        bus.register::<AccountCommandContext<PostgresTransactionManager>, SuspendCommand, SuspendHandler<PostgresTransactionManager>>(SuspendHandler::new());
        bus.register::<AccountCommandContext<PostgresTransactionManager>, UnsuspendCommand, UnsuspendHandler<PostgresTransactionManager>>(UnsuspendHandler::new());
        bus.register::<AccountCommandContext<PostgresTransactionManager>, BanCommand, BanHandler<PostgresTransactionManager>>(
            BanHandler::new(),
        );
        bus.register::<AccountCommandContext<PostgresTransactionManager>, UnbanCommand, UnbanHandler<PostgresTransactionManager>>(UnbanHandler::new());
        bus.register::<AccountCommandContext<PostgresTransactionManager>, ShadowbanCommand, ShadowbanHandler<PostgresTransactionManager>>(ShadowbanHandler::new());
        bus.register::<AccountCommandContext<PostgresTransactionManager>, LiftShadowbanCommand, LiftShadowbanHandler<PostgresTransactionManager>>(
            LiftShadowbanHandler::new(),
        );
        bus.register::<AccountCommandContext<PostgresTransactionManager>, IncreaseTrustScoreCommand, IncreaseTrustScoreHandler<PostgresTransactionManager>>(
            IncreaseTrustScoreHandler::new(),
        );
        bus.register::<AccountCommandContext<PostgresTransactionManager>, DecreaseTrustScoreCommand, DecreaseTrustScoreHandler<PostgresTransactionManager>>(
            DecreaseTrustScoreHandler::new(),
        );
        bus.register::<AccountCommandContext<PostgresTransactionManager>, AddPushTokenCommand, AddPushTokenHandler<PostgresTransactionManager>>(
            AddPushTokenHandler::new(),
        );
        bus.register::<AccountCommandContext<PostgresTransactionManager>, RemovePushTokenCommand, RemovePushTokenHandler<PostgresTransactionManager>>(
            RemovePushTokenHandler::new(),
        );
        bus.register::<AccountCommandContext<PostgresTransactionManager>, ChangeEmailCommand, ChangeEmailHandler<PostgresTransactionManager>>(
            ChangeEmailHandler::new(),
        );
        bus.register::<AccountCommandContext<PostgresTransactionManager>, ChangePhoneCommand, ChangePhoneNumberHandler<PostgresTransactionManager>>(
            ChangePhoneNumberHandler::new(),
        );
        bus.register::<AccountCommandContext<PostgresTransactionManager>, ChangeBirthDateCommand, ChangeBirthDateHandler<PostgresTransactionManager>>(
            ChangeBirthDateHandler::new(),
        );
        // bus.register::<AccountCommandContext<PostgresTransactionManager>, ChangeRegionCommand, ChangeRegionHandler>(
        //     ChangeRegionHandler,
        // );
        bus.register::<AccountCommandContext<PostgresTransactionManager>, UpdateLocaleCommand, UpdateLocaleHandler<PostgresTransactionManager>>(
            UpdateLocaleHandler::new(),
        );
        bus.register::<AccountCommandContext<PostgresTransactionManager>, UpdateTimezoneCommand, UpdateTimezoneHandler<PostgresTransactionManager>>(
            UpdateTimezoneHandler::new(),
        );
        bus.register::<AccountCommandContext<PostgresTransactionManager>, UpdatePreferencesCommand, UpdatePreferencesHandler<PostgresTransactionManager>>(
            UpdatePreferencesHandler::new(),
        );

        Arc::new(bus)
    }
}
