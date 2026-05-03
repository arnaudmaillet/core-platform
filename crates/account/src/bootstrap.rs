use std::sync::Arc;

use crate::{
    context::{AccountAppContext, AccountContext},
    db::PostgresAccountRepository,
    use_cases::{
        ActivateCommand, ActivateHandler, AddPushTokenCommand, AddPushTokenHandler, BanCommand,
        BanHandler, ChangeBirthDateCommand, ChangeBirthDateHandler, ChangeEmailCommand,
        ChangeEmailHandler, ChangePhoneNumberCommand, ChangePhoneNumberHandler,
        ChangeRegionCommand, ChangeRegionHandler, ChangeRoleCommand, ChangeRoleHandler,
        DeactivateCommand, DeactivateHandler, DecreaseTrustScoreCommand, DecreaseTrustScoreHandler,
        IncreaseTrustScoreCommand, IncreaseTrustScoreHandler, LiftShadowbanCommand,
        LiftShadowbanHandler, LinkSubIdentityCommand, LinkSubIdentityHandler, RegisterCommand,
        RegisterHandler, RemovePushTokenCommand, RemovePushTokenHandler, ShadowbanCommand,
        ShadowbanHandler, SuspendCommand, SuspendHandler, UnbanCommand, UnbanHandler,
        UnsuspendCommand, UnsuspendHandler, UpdateLocaleCommand, UpdateLocaleHandler,
        UpdatePreferencesCommand, UpdatePreferencesHandler, UpdateTimezoneCommand,
        UpdateTimezoneHandler,
    },
};
use shared_kernel::{
    application::{BaseAppContext, CommandBus},
    domain::repositories::CacheRepository,
    infrastructure::postgres::repositories::{
        PostgresIdempotencyRepository, PostgresOutboxRepository,
    },
};
use sqlx::PgPool;

pub struct AccountServiceBuilder {
    pool: PgPool,
    redis_repo: Arc<dyn CacheRepository>,
}

impl AccountServiceBuilder {
    pub fn new(pool: PgPool, redis_repo: Arc<dyn CacheRepository>) -> Self {
        Self { pool, redis_repo }
    }

    pub fn build_context(&self) -> Arc<AccountAppContext> {
        let account_repo = Arc::new(PostgresAccountRepository::new(
            self.pool.clone(),
            self.redis_repo.clone(),
        ));

        let outbox_repo = Arc::new(PostgresOutboxRepository::new(self.pool.clone()));
        let idempotency_repo = Arc::new(PostgresIdempotencyRepository::new("account_idempotency"));

        Arc::new(AccountAppContext::new(
            BaseAppContext::new(Some(self.pool.clone()), self.redis_repo.clone()),
            account_repo,
            outbox_repo,
            idempotency_repo,
        ))
    }

    pub fn build_command_bus(&self) -> Arc<CommandBus> {
        let mut bus = CommandBus::new();

        // --- Access Management ---
        bus.register::<AccountContext, RegisterCommand, RegisterHandler>(RegisterHandler);
        bus.register::<AccountContext, LinkSubIdentityCommand, LinkSubIdentityHandler>(
            LinkSubIdentityHandler,
        );

        // --- Lifecycle ---
        bus.register::<AccountContext, ActivateCommand, ActivateHandler>(ActivateHandler);
        bus.register::<AccountContext, DeactivateCommand, DeactivateHandler>(DeactivateHandler);
        bus.register::<AccountContext, ChangeRoleCommand, ChangeRoleHandler>(ChangeRoleHandler);
        bus.register::<AccountContext, SuspendCommand, SuspendHandler>(SuspendHandler);
        bus.register::<AccountContext, UnsuspendCommand, UnsuspendHandler>(UnsuspendHandler);

        // --- Moderation ---
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

        // --- Settings ---
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

        Arc::new(bus)
    }
}
