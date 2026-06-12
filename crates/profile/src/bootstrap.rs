// crates/profile/src/application/builder.rs

use shared_kernel::{
    cache::CacheRepository, command::CommandBus, idempotency::IdempotencyRepository, types::Region,
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
    repositories::{ProfileRepository, ProfileRoutingRepository},
};

pub struct ProfileServiceBuilder {
    profile_repo: Arc<dyn ProfileRepository>,
    routing_repo: Arc<dyn ProfileRoutingRepository>,
    cache_repo: Arc<dyn CacheRepository>,
    idempotency_repo: Arc<dyn IdempotencyRepository>,
    region: Region,
}

impl ProfileServiceBuilder {
    pub fn new(
        profile_repo: Arc<dyn ProfileRepository>,
        routing_repo: Arc<dyn ProfileRoutingRepository>,
        cache_repo: Arc<dyn CacheRepository>,
        idempotency_repo: Arc<dyn IdempotencyRepository>,
        region: Region,
    ) -> Self {
        Self {
            profile_repo,
            routing_repo,
            cache_repo,
            idempotency_repo,
            region,
        }
    }

    pub fn build_context(&self) -> ProfileAppContext {
        ProfileAppContext::new(
            self.profile_repo.clone(),
            self.routing_repo.clone(),
            self.region,
        )
    }

    pub fn build_command_bus(&self) -> Arc<CommandBus> {
        let mut bus = CommandBus::new(self.cache_repo.clone(), self.idempotency_repo.clone());

        bus.register::<ProfileCommandContext, CreateProfileCommand, CreateProfileHandler>(
            CreateProfileHandler::new(),
        );
        bus.register::<ProfileCommandContext, UpdateDisplayNameCommand, UpdateDisplayNameHandler>(
            UpdateDisplayNameHandler::new(),
        );
        bus.register::<ProfileCommandContext, ChangeHandleCommand, ChangeHandleHandler>(
            ChangeHandleHandler::new(),
        );
        bus.register::<ProfileCommandContext, UpdatePrivacyCommand, UpdatePrivacyHandler>(
            UpdatePrivacyHandler::new(),
        );
        bus.register::<ProfileCommandContext, UpdateAvatarCommand, UpdateAvatarHandler>(
            UpdateAvatarHandler::new(),
        );
        bus.register::<ProfileCommandContext, RemoveAvatarCommand, RemoveAvatarHandler>(
            RemoveAvatarHandler::new(),
        );
        bus.register::<ProfileCommandContext, UpdateBannerCommand, UpdateBannerHandler>(
            UpdateBannerHandler::new(),
        );
        bus.register::<ProfileCommandContext, RemoveBannerCommand, RemoveBannerHandler>(
            RemoveBannerHandler::new(),
        );
        bus.register::<ProfileCommandContext, UpdateBioCommand, UpdateBioHandler>(
            UpdateBioHandler::new(),
        );
        bus.register::<ProfileCommandContext, UpdateLocationCommand, UpdateLocationHandler>(
            UpdateLocationHandler::new(),
        );
        bus.register::<ProfileCommandContext, UpdateSocialsCommand, UpdateSocialsHandler>(
            UpdateSocialsHandler::new(),
        );

        Arc::new(bus)
    }
}
