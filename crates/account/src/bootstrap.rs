use std::sync::Arc;

use crate::{
    commands::{
        ActivateCommand, ActivateHandler, AddPushTokenCommand, AddPushTokenHandler, BanCommand,
        BanHandler, ChangeBirthDateCommand, ChangeBirthDateHandler, ChangeEmailCommand,
        ChangeEmailHandler, ChangePhoneNumberCommand, ChangePhoneNumberHandler,
        ChangeRoleCommand, ChangeRoleHandler,
        DeactivateCommand, DeactivateHandler, DecreaseTrustScoreCommand, DecreaseTrustScoreHandler,
        IncreaseTrustScoreCommand, IncreaseTrustScoreHandler, LiftShadowbanCommand,
        LiftShadowbanHandler, LinkSubIdentityCommand, LinkSubIdentityHandler, RegisterCommand,
        RegisterHandler, RemovePushTokenCommand, RemovePushTokenHandler, ShadowbanCommand,
        ShadowbanHandler, SuspendCommand, SuspendHandler, UnbanCommand, UnbanHandler,
        UnsuspendCommand, UnsuspendHandler, UpdateLocaleCommand, UpdateLocaleHandler,
        UpdatePreferencesCommand, UpdatePreferencesHandler, UpdateTimezoneCommand,
        UpdateTimezoneHandler,
    },
    context::{AccountAppContext, AccountCommandContext},
    db::PostgresAccountRepository,
};
use infra_sqlx::{PostgresIdempotencyRepository, PostgresOutboxRepository, sqlx::PgPool};
use shared_kernel::{cache::CacheRepository, command::CommandBus};

pub struct AccountServiceBuilder {
    pool: PgPool,
    cache_repo: Arc<dyn CacheRepository>,
}

impl AccountServiceBuilder {
    pub fn new(pool: PgPool, cache_repo: Arc<dyn CacheRepository>) -> Self {
        Self { pool, cache_repo }
    }

    pub fn build_context(&self) -> Arc<AccountAppContext> {
        let account_repo = Arc::new(PostgresAccountRepository::new(self.pool.clone()));

        let outbox_repo = Arc::new(PostgresOutboxRepository::new(self.pool.clone()));
        let idempotency_repo = Arc::new(PostgresIdempotencyRepository::new("account_idempotency"));

        Arc::new(AccountAppContext::new(
            self.pool.clone(),
            account_repo,
            outbox_repo,
            idempotency_repo,
        ))
    }

    pub fn build_command_bus(&self) -> Arc<CommandBus> {
        let mut bus = CommandBus::new(self.cache_repo.clone());

        // --- Access Management ---
        bus.register::<AccountCommandContext, RegisterCommand, RegisterHandler>(RegisterHandler);
        bus.register::<AccountCommandContext, LinkSubIdentityCommand, LinkSubIdentityHandler>(
            LinkSubIdentityHandler,
        );

        // --- Lifecycle ---
        bus.register::<AccountCommandContext, ActivateCommand, ActivateHandler>(ActivateHandler);
        bus.register::<AccountCommandContext, DeactivateCommand, DeactivateHandler>(DeactivateHandler);
        bus.register::<AccountCommandContext, ChangeRoleCommand, ChangeRoleHandler>(ChangeRoleHandler);
        bus.register::<AccountCommandContext, SuspendCommand, SuspendHandler>(SuspendHandler);
        bus.register::<AccountCommandContext, UnsuspendCommand, UnsuspendHandler>(UnsuspendHandler);

        // --- Moderation ---
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

        // --- Settings ---
        bus.register::<AccountCommandContext, AddPushTokenCommand, AddPushTokenHandler>(
            AddPushTokenHandler,
        );
        bus.register::<AccountCommandContext, RemovePushTokenCommand, RemovePushTokenHandler>(
            RemovePushTokenHandler,
        );
        bus.register::<AccountCommandContext, ChangeEmailCommand, ChangeEmailHandler>(ChangeEmailHandler);
        bus.register::<AccountCommandContext, ChangePhoneNumberCommand, ChangePhoneNumberHandler>(
            ChangePhoneNumberHandler,
        );
        bus.register::<AccountCommandContext, ChangeBirthDateCommand, ChangeBirthDateHandler>(
            ChangeBirthDateHandler,
        );
        // bus.register::<AccountCommandContext, ChangeRegionCommand, ChangeRegionHandler>(
        //     ChangeRegionHandler,
        // );
        bus.register::<AccountCommandContext, UpdateLocaleCommand, UpdateLocaleHandler>(
            UpdateLocaleHandler,
        );
        bus.register::<AccountCommandContext, UpdateTimezoneCommand, UpdateTimezoneHandler>(
            UpdateTimezoneHandler,
        );
        bus.register::<AccountCommandContext, UpdatePreferencesCommand, UpdatePreferencesHandler>(
            UpdatePreferencesHandler,
        );

        Arc::new(bus)
    }
}
