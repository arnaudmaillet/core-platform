// crates/profile/src/application/builder.rs

use infra_sqlx::sqlx::PgPool;
use infra_sqlx::{PostgresIdempotencyRepository, PostgresOutboxRepository};
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

    pub fn build_context(&self) -> Arc<ProfileAppContext> {
        let profile_repo = Arc::new(PostgresProfileRepository::new(self.pool.clone()));
        let outbox_repo = Arc::new(PostgresOutboxRepository::new(self.pool.clone()));
        let idempotency_repo = Arc::new(PostgresIdempotencyRepository::new("profile_idempotency"));

        Arc::new(ProfileAppContext::new(
            self.pool.clone(),
            profile_repo,
            outbox_repo,
            idempotency_repo,
        ))
    }

    pub fn build_command_bus(&self) -> Arc<CommandBus> {
        let mut bus = CommandBus::new(self.redis_repo.clone());

        bus.register::<ProfileCommandContext, CreateProfileCommand, CreateProfileHandler>(
            CreateProfileHandler,
        );
        bus.register::<ProfileCommandContext, UpdateDisplayNameCommand, UpdateDisplayNameHandler>(
            UpdateDisplayNameHandler,
        );
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
        bus.register::<ProfileCommandContext, UpdateBioCommand, UpdateBioHandler>(UpdateBioHandler);
        bus.register::<ProfileCommandContext, UpdateLocationCommand, UpdateLocationHandler>(
            UpdateLocationHandler,
        );
        bus.register::<ProfileCommandContext, UpdateSocialsCommand, UpdateSocialsHandler>(
            UpdateSocialsHandler,
        );

        Arc::new(bus)
    }
}
