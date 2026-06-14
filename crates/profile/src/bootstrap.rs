// crates/profile/src/application/builder.rs

use shared_kernel::{
    command::CommandBus, environment::ClusterContext, idempotency::IdempotencyRepository,
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
    context::{ProfileCommandCtx, ProfileKernelCtx},
    repositories::{ProfileRepository, ProfileRoutingRepository},
};

pub struct ProfileServiceBuilder {
    profile_repo: Arc<dyn ProfileRepository>,
    routing_repo: Arc<dyn ProfileRoutingRepository>,
    idempotency_repo: Arc<dyn IdempotencyRepository>,
    cluster_ctx: ClusterContext,
}

impl ProfileServiceBuilder {
    pub fn new(
        profile_repo: Arc<dyn ProfileRepository>,
        routing_repo: Arc<dyn ProfileRoutingRepository>,
        idempotency_repo: Arc<dyn IdempotencyRepository>,
        cluster_ctx: ClusterContext,
    ) -> Self {
        Self {
            profile_repo,
            routing_repo,
            idempotency_repo,
            cluster_ctx,
        }
    }

    pub fn build_kernel_ctx(&self) -> ProfileKernelCtx {
        ProfileKernelCtx::new(
            self.profile_repo.clone(),
            self.routing_repo.clone(),
            self.idempotency_repo.clone(),
            self.cluster_ctx,
        )
    }

    pub fn register_handlers(&self, bus: &mut CommandBus) {
        bus.register::<ProfileCommandCtx, CreateProfileCommand, CreateProfileHandler>(
            CreateProfileHandler::new(),
        );
        bus.register::<ProfileCommandCtx, UpdateDisplayNameCommand, UpdateDisplayNameHandler>(
            UpdateDisplayNameHandler::new(),
        );
        bus.register::<ProfileCommandCtx, ChangeHandleCommand, ChangeHandleHandler>(
            ChangeHandleHandler::new(),
        );
        bus.register::<ProfileCommandCtx, UpdatePrivacyCommand, UpdatePrivacyHandler>(
            UpdatePrivacyHandler::new(),
        );
        bus.register::<ProfileCommandCtx, UpdateAvatarCommand, UpdateAvatarHandler>(
            UpdateAvatarHandler::new(),
        );
        bus.register::<ProfileCommandCtx, RemoveAvatarCommand, RemoveAvatarHandler>(
            RemoveAvatarHandler::new(),
        );
        bus.register::<ProfileCommandCtx, UpdateBannerCommand, UpdateBannerHandler>(
            UpdateBannerHandler::new(),
        );
        bus.register::<ProfileCommandCtx, RemoveBannerCommand, RemoveBannerHandler>(
            RemoveBannerHandler::new(),
        );
        bus.register::<ProfileCommandCtx, UpdateBioCommand, UpdateBioHandler>(
            UpdateBioHandler::new(),
        );
        bus.register::<ProfileCommandCtx, UpdateLocationCommand, UpdateLocationHandler>(
            UpdateLocationHandler::new(),
        );
        bus.register::<ProfileCommandCtx, UpdateSocialsCommand, UpdateSocialsHandler>(
            UpdateSocialsHandler::new(),
        );
    }
}
