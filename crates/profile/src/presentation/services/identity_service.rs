// crates/profile/src/presentation/services/profile_identity_service.rs

use crate::commands::{
    ChangeHandleCommand, CreateProfileCommand, UpdateDisplayNameCommand, UpdatePrivacyCommand,
};
use crate::context::ProfileAppContext;
use crate::presentation::utils::shared::GrpcServiceUtils;
use shared_kernel::command::CommandBus;
use shared_kernel::core::Identifier;
use shared_kernel::types::ProfileId;
use shared_proto::profile::v1::profile_identity_service_server::ProfileIdentityService as ProtoProfileIdentityService;
use shared_proto::profile::v1::{
    ChangeHandleRequest, ChangeHandleResponse, CreateProfileRequest, CreateProfileResponse,
    UpdateDisplayNameRequest, UpdateDisplayNameResponse, UpdatePrivacyRequest,
    UpdatePrivacyResponse,
};
use std::sync::Arc;
use tonic::{Request, Response, Status};

pub struct ProfileIdentityService {
    bus: Arc<CommandBus>,
    app_ctx: Arc<ProfileAppContext>,
}

impl ProfileIdentityService {
    pub fn new(bus: Arc<CommandBus>, app_ctx: Arc<ProfileAppContext>) -> Self {
        Self { bus, app_ctx }
    }
}

impl GrpcServiceUtils for ProfileIdentityService {
    fn app_ctx(&self) -> &ProfileAppContext {
        &self.app_ctx
    }
    fn bus(&self) -> &CommandBus {
        &self.bus
    }
}

#[tonic::async_trait]
impl ProtoProfileIdentityService for ProfileIdentityService {
    async fn create_profile(
        &self,
        request: Request<CreateProfileRequest>,
    ) -> Result<Response<CreateProfileResponse>, Status> {
        let (_, extensions, req_inner) = request.into_parts();
        let generated_profile_id = ProfileId::generate();

        let ctx = self.build_creation_context(&extensions)?;
        let command = CreateProfileCommand::try_from_proto(req_inner, generated_profile_id)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        self.dispatch_command::<CreateProfileCommand, (), CreateProfileResponse>(
            &ctx,
            command,
            CreateProfileResponse {
                profile_id: generated_profile_id.to_string(),
            },
        )
        .await
    }

    async fn update_display_name(
        &self,
        request: Request<UpdateDisplayNameRequest>,
    ) -> Result<Response<UpdateDisplayNameResponse>, Status> {
        let (_metadata, extensions, req_inner) = request.into_parts();
        let target = req_inner
            .target
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Missing target context"))?;

        let profile_id =
            ProfileId::from_uuid(uuid::Uuid::parse_str(&target.profile_id).map_err(|e| {
                Status::invalid_argument(format!("Invalid profile_id format: {}", e))
            })?);

        let ctx = self.build_command_context(profile_id, &extensions)?;
        let command = UpdateDisplayNameCommand::try_from_proto(req_inner)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        self.dispatch_command::<UpdateDisplayNameCommand, (), UpdateDisplayNameResponse>(
            &ctx,
            command,
            UpdateDisplayNameResponse {},
        )
        .await
    }

    async fn change_handle(
        &self,
        request: Request<ChangeHandleRequest>,
    ) -> Result<Response<ChangeHandleResponse>, Status> {
        let (_metadata, ext, req_inner) = request.into_parts();
        let target = req_inner
            .target
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Missing target context"))?;

        let profile_id =
            ProfileId::from_uuid(uuid::Uuid::parse_str(&target.profile_id).map_err(|e| {
                Status::invalid_argument(format!("Invalid profile_id format: {}", e))
            })?);

        let ctx = self.build_command_context(profile_id, &ext)?;
        let command = ChangeHandleCommand::try_from_proto(req_inner)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        self.dispatch_command::<ChangeHandleCommand, (), ChangeHandleResponse>(
            &ctx,
            command,
            ChangeHandleResponse {},
        )
        .await
    }

    async fn update_privacy(
        &self,
        request: Request<UpdatePrivacyRequest>,
    ) -> Result<Response<UpdatePrivacyResponse>, Status> {
        let (_metadata, extensions, req_inner) = request.into_parts();
        let target = req_inner
            .target
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Missing target context"))?;

        let profile_id =
            ProfileId::from_uuid(uuid::Uuid::parse_str(&target.profile_id).map_err(|e| {
                Status::invalid_argument(format!("Invalid profile_id format: {}", e))
            })?);

        let ctx = self.build_command_context(profile_id, &extensions)?;
        let command = UpdatePrivacyCommand::try_from_proto(req_inner)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        self.dispatch_command::<UpdatePrivacyCommand, (), UpdatePrivacyResponse>(
            &ctx,
            command,
            UpdatePrivacyResponse {},
        )
        .await
    }
}
