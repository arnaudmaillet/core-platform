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
    context::{ProfileAppContext, ProfileContext},
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

    /// Construit le contexte global de l'application Profile
    pub fn build_context(&self) -> Arc<ProfileAppContext> {
        let profile_repo = Arc::new(PostgresProfileRepository::new(self.pool.clone()));

        let outbox_repo = Arc::new(PostgresOutboxRepository::new(self.pool.clone()));
        // Note: Utilisation d'un scope d'idempotence spécifique au profil
        let idempotency_repo = Arc::new(PostgresIdempotencyRepository::new("profile_idempotency"));

        Arc::new(ProfileAppContext::new(
            self.pool.clone(),
            profile_repo,
            outbox_repo,
            idempotency_repo,
        ))
    }

    /// Enregistre tous les handlers dans le CommandBus
    pub fn build_command_bus(&self) -> Arc<CommandBus> {
        let mut bus = CommandBus::new(self.redis_repo.clone());

        // --- Identity Section ---
        bus.register::<ProfileContext, CreateProfileCommand, CreateProfileHandler>(
            CreateProfileHandler,
        );
        bus.register::<ProfileContext, UpdateDisplayNameCommand, UpdateDisplayNameHandler>(
            UpdateDisplayNameHandler,
        );
        bus.register::<ProfileContext, ChangeHandleCommand, ChangeHandleHandler>(
            ChangeHandleHandler,
        );
        bus.register::<ProfileContext, UpdatePrivacyCommand, UpdatePrivacyHandler>(
            UpdatePrivacyHandler,
        );

        // --- Media Section ---
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

        // --- Metadata Section ---
        bus.register::<ProfileContext, UpdateBioCommand, UpdateBioHandler>(UpdateBioHandler);
        bus.register::<ProfileContext, UpdateLocationCommand, UpdateLocationHandler>(
            UpdateLocationHandler,
        );
        bus.register::<ProfileContext, UpdateSocialsCommand, UpdateSocialsHandler>(
            UpdateSocialsHandler,
        );

        Arc::new(bus)
    }
}
