// crates/profile/src/presentation/services/profile_metadata_service.rs

use crate::commands::{UpdateBioCommand, UpdateLocationCommand, UpdateSocialsCommand};
use crate::context::ProfileKernelCtx;
use crate::presentation::utils::shared::GrpcServiceUtils;
use shared_kernel::command::CommandBus;
use shared_kernel::core::Identifier;
use shared_kernel::types::ProfileId;
use shared_proto::profile::v1::profile_metadata_service_server::ProfileMetadataService as ProtoProfileMetadataService;
use shared_proto::profile::v1::{
    UpdateBioRequest, UpdateBioResponse, UpdateLocationRequest, UpdateLocationResponse,
    UpdateSocialsRequest, UpdateSocialsResponse,
};
use std::sync::Arc;
use tonic::{Request, Response, Status};

pub struct ProfileMetadataService {
    bus: Arc<CommandBus>,
    app_ctx: ProfileKernelCtx,
}

impl ProfileMetadataService {
    pub fn new(bus: Arc<CommandBus>, app_ctx: ProfileKernelCtx) -> Self {
        Self { bus, app_ctx }
    }
}

impl GrpcServiceUtils for ProfileMetadataService {
    fn kernel(&self) -> &ProfileKernelCtx {
        &self.app_ctx
    }
    fn bus(&self) -> &CommandBus {
        &self.bus
    }
}

#[tonic::async_trait]
impl ProtoProfileMetadataService for ProfileMetadataService {
    async fn update_bio(
        &self,
        request: Request<UpdateBioRequest>,
    ) -> Result<Response<UpdateBioResponse>, Status> {
        let (_, extensions, req_inner) = request.into_parts();
        let target = req_inner
            .target
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Missing target context"))?;

        let profile_id =
            ProfileId::from_uuid(uuid::Uuid::parse_str(&target.profile_id).map_err(|e| {
                Status::invalid_argument(format!("Invalid profile_id format: {}", e))
            })?);

        let ctx = self.build_command_ctx(profile_id, &extensions)?;
        let command = UpdateBioCommand::try_from_proto(req_inner)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        self.dispatch_command::<UpdateBioCommand, (), UpdateBioResponse>(
            &ctx,
            command,
            UpdateBioResponse {},
        )
        .await
    }

    async fn update_location(
        &self,
        request: Request<UpdateLocationRequest>,
    ) -> Result<Response<UpdateLocationResponse>, Status> {
        let (_, extensions, req_inner) = request.into_parts();
        let target = req_inner
            .target
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Missing target context"))?;

        let profile_id =
            ProfileId::from_uuid(uuid::Uuid::parse_str(&target.profile_id).map_err(|e| {
                Status::invalid_argument(format!("Invalid profile_id format: {}", e))
            })?);

        let ctx = self.build_command_ctx(profile_id, &extensions)?;
        let command = UpdateLocationCommand::try_from_proto(req_inner)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        self.dispatch_command::<UpdateLocationCommand, (), UpdateLocationResponse>(
            &ctx,
            command,
            UpdateLocationResponse {},
        )
        .await
    }

    async fn update_socials(
        &self,
        request: Request<UpdateSocialsRequest>,
    ) -> Result<Response<UpdateSocialsResponse>, Status> {
        let (_, extensions, req_inner) = request.into_parts();
        let target = req_inner
            .target
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Missing target context"))?;

        let profile_id =
            ProfileId::from_uuid(uuid::Uuid::parse_str(&target.profile_id).map_err(|e| {
                Status::invalid_argument(format!("Invalid profile_id format: {}", e))
            })?);

        let ctx = self.build_command_ctx(profile_id, &extensions)?;
        let command = UpdateSocialsCommand::try_from_proto(req_inner)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        self.dispatch_command::<UpdateSocialsCommand, (), UpdateSocialsResponse>(
            &ctx,
            command,
            UpdateSocialsResponse {},
        )
        .await
    }
}
