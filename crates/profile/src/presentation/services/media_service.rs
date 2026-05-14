// crates/profile/src/presentation/services/profile_media_service.rs

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
use crate::presentation::utils::mapper::map_profile_to_proto;
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
        // 1. Extraire l'ID sans consommer la request
        let profile_id = request
            .get_ref()
            .target
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Missing target"))?
            .profile_id
            .parse()
            .map_err(|e| Status::invalid_argument(format!("Invalid profile_id: {}", e)))?;

        // 2. Contexte synchrone
        let ctx = self.get_context(&request, &profile_id)?;

        // 3. Traitement
        let req = request.into_inner();
        let command = UpdateAvatarCommand::try_from_proto(req)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        self.execute_and_fetch::<UpdateAvatarCommand, (), UpdateAvatarResponse, _>(
            &ctx,
            command,
            |profile| UpdateAvatarResponse {
                profile: Some(map_profile_to_proto(profile)),
            },
        )
        .await
    }

    async fn update_banner(
        &self,
        request: Request<UpdateBannerRequest>,
    ) -> Result<Response<UpdateBannerResponse>, Status> {
        let profile_id = request
            .get_ref()
            .target
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Missing target"))?
            .profile_id
            .parse()
            .map_err(|e| Status::invalid_argument(format!("Invalid profile_id: {}", e)))?;

        let ctx = self.get_context(&request, &profile_id)?;

        let req = request.into_inner();
        let command = UpdateBannerCommand::try_from_proto(req)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        self.execute_and_fetch::<UpdateBannerCommand, (), UpdateBannerResponse, _>(
            &ctx,
            command,
            |profile| UpdateBannerResponse {
                profile: Some(map_profile_to_proto(profile)),
            },
        )
        .await
    }

    async fn remove_avatar(
        &self,
        request: Request<RemoveAvatarRequest>,
    ) -> Result<Response<RemoveAvatarResponse>, Status> {
        let profile_id = request
            .get_ref()
            .target
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Missing target"))?
            .profile_id
            .parse()
            .map_err(|e| Status::invalid_argument(format!("Invalid profile_id: {}", e)))?;

        let ctx = self.get_context(&request, &profile_id)?;

        let req = request.into_inner();
        let command = RemoveAvatarCommand::try_from_proto(req)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        self.execute_and_fetch::<RemoveAvatarCommand, (), RemoveAvatarResponse, _>(
            &ctx,
            command,
            |profile| RemoveAvatarResponse {
                profile: Some(map_profile_to_proto(profile)),
            },
        )
        .await
    }

    async fn remove_banner(
        &self,
        request: Request<RemoveBannerRequest>,
    ) -> Result<Response<RemoveBannerResponse>, Status> {
        let profile_id = request
            .get_ref()
            .target
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Missing target"))?
            .profile_id
            .parse()
            .map_err(|e| Status::invalid_argument(format!("Invalid profile_id: {}", e)))?;

        let ctx = self.get_context(&request, &profile_id)?;

        let req = request.into_inner();
        let command = RemoveBannerCommand::try_from_proto(req)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        self.execute_and_fetch::<RemoveBannerCommand, (), RemoveBannerResponse, _>(
            &ctx,
            command,
            |profile| RemoveBannerResponse {
                profile: Some(map_profile_to_proto(profile)),
            },
        )
        .await
    }
}
