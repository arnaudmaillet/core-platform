// crates/profile/src/presentation/services/profile_metadata_service.rs

use shared_proto::profile::v1::profile_metadata_service_server::ProfileMetadataService as ProtoProfileMetadataService;
use shared_proto::profile::v1::{
    UpdateBioRequest, UpdateBioResponse, UpdateLocationRequest, UpdateLocationResponse,
    UpdateSocialsRequest, UpdateSocialsResponse,
};
use std::sync::Arc;
use tonic::{Request, Response, Status};

// Kernel & Application imports
use crate::commands::{UpdateBioCommand, UpdateLocationCommand, UpdateSocialsCommand};
use crate::context::ProfileAppContext;
use crate::presentation::utils::shared::GrpcServiceUtils;
use crate::types::ProfileId;
use shared_kernel::command::CommandBus;

pub struct ProfileMetadataService {
    bus: Arc<CommandBus>,
    app_ctx: Arc<ProfileAppContext>,
}

impl ProfileMetadataService {
    pub fn new(bus: Arc<CommandBus>, app_ctx: Arc<ProfileAppContext>) -> Self {
        Self { bus, app_ctx }
    }
}

impl GrpcServiceUtils for ProfileMetadataService {
    fn app_ctx(&self) -> &ProfileAppContext {
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
        let profile_id = target
            .profile_id
            .parse::<ProfileId>()
            .map_err(|e| Status::invalid_argument(format!("Invalid profile_id format: {}", e)))?;
        let ctx = self.build_context(profile_id, &extensions)?;
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
        let profile_id = target
            .profile_id
            .parse::<ProfileId>()
            .map_err(|e| Status::invalid_argument(format!("Invalid profile_id format: {}", e)))?;
        let ctx = self.build_context(profile_id, &extensions)?;
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
        let profile_id = target
            .profile_id
            .parse::<ProfileId>()
            .map_err(|e| Status::invalid_argument(format!("Invalid profile_id format: {}", e)))?;
        let ctx = self.build_context(profile_id, &extensions)?;
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
