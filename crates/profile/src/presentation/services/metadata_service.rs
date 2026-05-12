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
use crate::presentation::utils::mapper::map_profile_to_proto;
use crate::presentation::utils::shared::GrpcServiceUtils;
use shared_kernel::application::CommandBus;

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
        let command = UpdateBioCommand::try_from_proto(req)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        self.execute_and_fetch::<UpdateBioCommand, (), UpdateBioResponse, _>(
            &ctx,
            command,
            |profile| UpdateBioResponse {
                profile: Some(map_profile_to_proto(profile)),
            },
        )
        .await
    }

    async fn update_location(
        &self,
        request: Request<UpdateLocationRequest>,
    ) -> Result<Response<UpdateLocationResponse>, Status> {
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
        let command = UpdateLocationCommand::try_from_proto(req)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        self.execute_and_fetch::<UpdateLocationCommand, (), UpdateLocationResponse, _>(
            &ctx,
            command,
            |profile| UpdateLocationResponse {
                profile: Some(map_profile_to_proto(profile)),
            },
        )
        .await
    }

    async fn update_socials(
        &self,
        request: Request<UpdateSocialsRequest>,
    ) -> Result<Response<UpdateSocialsResponse>, Status> {
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
        let command = UpdateSocialsCommand::try_from_proto(req)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        self.execute_and_fetch::<UpdateSocialsCommand, (), UpdateSocialsResponse, _>(
            &ctx,
            command,
            |profile| UpdateSocialsResponse {
                profile: Some(map_profile_to_proto(profile)),
            },
        )
        .await
    }
}
