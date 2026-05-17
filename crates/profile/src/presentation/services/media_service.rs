// crates/profile/src/presentation/services/profile_media_service.rs

use shared_kernel::types::ProfileId;
use shared_proto::profile::v1::profile_media_service_server::ProfileMediaService as ProtoProfileMediaService;
use shared_proto::profile::v1::{
    RemoveAvatarRequest, RemoveAvatarResponse, RemoveBannerRequest, RemoveBannerResponse,
    UpdateAvatarRequest, UpdateAvatarResponse, UpdateBannerRequest, UpdateBannerResponse,
};
use std::sync::Arc;
use tonic::{Request, Response, Status};

// Kernel & Application imports
use crate::commands::{
    RemoveAvatarCommand, RemoveBannerCommand, UpdateAvatarCommand, UpdateBannerCommand,
};
use crate::context::ProfileAppContext;
use crate::presentation::utils::shared::GrpcServiceUtils;
use shared_kernel::command::CommandBus;

pub struct ProfileMediaService {
    bus: Arc<CommandBus>,
    app_ctx: Arc<ProfileAppContext>,
}

impl ProfileMediaService {
    pub fn new(bus: Arc<CommandBus>, app_ctx: Arc<ProfileAppContext>) -> Self {
        Self { bus, app_ctx }
    }
}

impl GrpcServiceUtils for ProfileMediaService {
    fn app_ctx(&self) -> &ProfileAppContext {
        &self.app_ctx
    }
    fn bus(&self) -> &CommandBus {
        &self.bus
    }
}

#[tonic::async_trait]
impl ProtoProfileMediaService for ProfileMediaService {
    async fn update_avatar(
        &self,
        request: Request<UpdateAvatarRequest>,
    ) -> Result<Response<UpdateAvatarResponse>, Status> {
        let (_, extensions, req_inner) = request.into_parts();
        let target = req_inner
            .target
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Missing target context"))?;
        let profile_id = target
            .profile_id
            .parse::<ProfileId>()
            .map_err(|e| Status::invalid_argument(format!("Invalid profile_id format: {}", e)))?;
        let ctx = self.build_context(profile_id, &extensions)?;
        let command = UpdateAvatarCommand::try_from_proto(req_inner)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        self.dispatch_command::<UpdateAvatarCommand, (), UpdateAvatarResponse>(
            &ctx,
            command,
            UpdateAvatarResponse {},
        )
        .await
    }

    async fn update_banner(
        &self,
        request: Request<UpdateBannerRequest>,
    ) -> Result<Response<UpdateBannerResponse>, Status> {
        let (_, extensions, req_inner) = request.into_parts();

        let target = req_inner
            .target
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Missing target context"))?;
        let profile_id = target
            .profile_id
            .parse::<ProfileId>()
            .map_err(|e| Status::invalid_argument(format!("Invalid profile_id format: {}", e)))?;
        let ctx = self.build_context(profile_id, &extensions)?;
        let command = UpdateBannerCommand::try_from_proto(req_inner)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        self.dispatch_command::<UpdateBannerCommand, (), UpdateBannerResponse>(
            &ctx,
            command,
            UpdateBannerResponse {},
        )
        .await
    }

    async fn remove_avatar(
        &self,
        request: Request<RemoveAvatarRequest>,
    ) -> Result<Response<RemoveAvatarResponse>, Status> {
        let (_, extensions, req_inner) = request.into_parts();
        let target = req_inner
            .target
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Missing target context"))?;
        let profile_id = target
            .profile_id
            .parse::<ProfileId>()
            .map_err(|e| Status::invalid_argument(format!("Invalid profile_id format: {}", e)))?;
        let ctx = self.build_context(profile_id, &extensions)?;
        let command = RemoveAvatarCommand::try_from_proto(req_inner)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        self.dispatch_command::<RemoveAvatarCommand, (), RemoveAvatarResponse>(
            &ctx,
            command,
            RemoveAvatarResponse {},
        )
        .await
    }

    async fn remove_banner(
        &self,
        request: Request<RemoveBannerRequest>,
    ) -> Result<Response<RemoveBannerResponse>, Status> {
        let (_, extensions, req_inner) = request.into_parts();
        let target = req_inner
            .target
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Missing target context"))?;
        let profile_id = target
            .profile_id
            .parse::<ProfileId>()
            .map_err(|e| Status::invalid_argument(format!("Invalid profile_id format: {}", e)))?;
        let ctx = self.build_context(profile_id, &extensions)?;
        let command = RemoveBannerCommand::try_from_proto(req_inner)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        self.dispatch_command::<RemoveBannerCommand, (), RemoveBannerResponse>(
            &ctx,
            command,
            RemoveBannerResponse {},
        )
        .await
    }
}
