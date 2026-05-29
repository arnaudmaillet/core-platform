// crates/profile/src/application/builder.rs

use infra_sqlx::sqlx::PgPool;
use infra_sqlx::{
    PostgresIdempotencyRepository, PostgresOutboxRepository, PostgresTransactionManager,
};
use std::sync::Arc;

use crate::{
    commands::{
        ChangeHandleCommand, ChangeHandleHandler, CreateProfileCommand, CreateProfileHandler,
        RemoveAvatarCommand, RemoveAvatarHandler, RemoveBannerCommand, RemoveBannerHandler,
        UpdateAvatarCommand, UpdateAvatarHandler, UpdateBannerCommand, UpdateBannerHandler,
        UpdateBioCommand, UpdateBioHandler, UpdateDisplayNameCommand, UpdateDisplayNameHandler,
        UpdateLocationCommand, UpdateLocationHandler, UpdatePrivacyCommand, UpdatePrivacyHandler,
        UpdateSocialsCommand, UpdateSocialsHandler,
    },
    context::{ProfileAppContext, ProfileCommandContext},
    repositories_impl::PostgresProfileRepository,
};

use shared_kernel::{cache::CacheRepository, command::CommandBus};

pub struct ProfileServiceBuilder {
    pool: PgPool,
    redis_repo: Arc<dyn CacheRepository>,
}

impl ProfileServiceBuilder {
    pub fn new(pool: PgPool, redis_repo: Arc<dyn CacheRepository>) -> Self {
        Self { pool, redis_repo }
    }

    pub fn build_context(&self) -> Arc<ProfileAppContext<PostgresTransactionManager>> {
        let tx_manager = Arc::new(PostgresTransactionManager::new(self.pool.clone()));
        let profile_repo = Arc::new(PostgresProfileRepository::new(self.pool.clone()));
        let outbox_repo = Arc::new(PostgresOutboxRepository::new(self.pool.clone()));
        let idempotency_repo = Arc::new(PostgresIdempotencyRepository::new("profile_idempotency"));

        Arc::new(ProfileAppContext::new(
            tx_manager,
            profile_repo,
            outbox_repo,
            idempotency_repo,
        ))
    }

    pub fn build_command_bus(&self) -> Arc<CommandBus> {
        let mut bus = CommandBus::new(self.redis_repo.clone());
        bus.register::<ProfileCommandContext<PostgresTransactionManager>, CreateProfileCommand, CreateProfileHandler<PostgresTransactionManager>>(
            CreateProfileHandler::new(),
        );
        bus.register::<ProfileCommandContext<PostgresTransactionManager>, UpdateDisplayNameCommand, UpdateDisplayNameHandler<PostgresTransactionManager>>(
            UpdateDisplayNameHandler::new(),
        );
        bus.register::<ProfileCommandContext<PostgresTransactionManager>, ChangeHandleCommand, ChangeHandleHandler<PostgresTransactionManager>>(
            ChangeHandleHandler::new(),
        );
        bus.register::<ProfileCommandContext<PostgresTransactionManager>, UpdatePrivacyCommand, UpdatePrivacyHandler<PostgresTransactionManager>>(
            UpdatePrivacyHandler::new(),
        );
        bus.register::<ProfileCommandContext<PostgresTransactionManager>, UpdateAvatarCommand, UpdateAvatarHandler<PostgresTransactionManager>>(
            UpdateAvatarHandler::new(),
        );
        bus.register::<ProfileCommandContext<PostgresTransactionManager>, RemoveAvatarCommand, RemoveAvatarHandler<PostgresTransactionManager>>(
            RemoveAvatarHandler::new(),
        );
        bus.register::<ProfileCommandContext<PostgresTransactionManager>, UpdateBannerCommand, UpdateBannerHandler<PostgresTransactionManager>>(
            UpdateBannerHandler::new(),
        );
        bus.register::<ProfileCommandContext<PostgresTransactionManager>, RemoveBannerCommand, RemoveBannerHandler<PostgresTransactionManager>>(
            RemoveBannerHandler::new(),
        );
        bus.register::<ProfileCommandContext<PostgresTransactionManager>, UpdateBioCommand, UpdateBioHandler<PostgresTransactionManager>>(
            UpdateBioHandler::new()
        );
        bus.register::<ProfileCommandContext<PostgresTransactionManager>, UpdateLocationCommand, UpdateLocationHandler<PostgresTransactionManager>>(
            UpdateLocationHandler::new(),
        );
        bus.register::<ProfileCommandContext<PostgresTransactionManager>, UpdateSocialsCommand, UpdateSocialsHandler<PostgresTransactionManager>>(
            UpdateSocialsHandler::new(),
        );

        Arc::new(bus)
    }
}
