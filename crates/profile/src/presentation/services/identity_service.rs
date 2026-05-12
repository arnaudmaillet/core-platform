// crates/profile/src/presentation/services/profile_identity_service.rs

use shared_proto::profile::v1::profile_identity_service_server::ProfileIdentityService as ProtoProfileIdentityService;
use shared_proto::profile::v1::{
    ChangeHandleRequest, ChangeHandleResponse, UpdateDisplayNameRequest, UpdateDisplayNameResponse,
    UpdatePrivacyRequest, UpdatePrivacyResponse,
};
use std::sync::Arc;
use tonic::{Request, Response, Status};

// Kernel & Application imports
use crate::commands::{ChangeHandleCommand, UpdateDisplayNameCommand, UpdatePrivacyCommand};
use crate::context::ProfileAppContext;
use crate::presentation::utils::mapper::map_profile_to_proto;
use crate::presentation::utils::shared::GrpcServiceUtils;
use shared_kernel::application::CommandBus;

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
    async fn update_display_name(
        &self,
        request: Request<UpdateDisplayNameRequest>,
    ) -> Result<Response<UpdateDisplayNameResponse>, Status> {
        let proto_req = request.get_ref();

        let target = proto_req
            .target
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Missing target"))?;
        let profile_id = target
            .profile_id
            .parse()
            .map_err(|e| Status::invalid_argument(format!("Invalid profile_id: {}", e)))?;

        let ctx = self.get_context(&request, &profile_id)?;

        let req = request.into_inner();
        let command = UpdateDisplayNameCommand::try_from_proto(req)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        self.execute_and_fetch::<UpdateDisplayNameCommand, (), UpdateDisplayNameResponse, _>(
            &ctx,
            command,
            |profile| UpdateDisplayNameResponse {
                profile: Some(map_profile_to_proto(profile)),
            },
        )
        .await
    }

    async fn change_handle(
        &self,
        request: Request<ChangeHandleRequest>,
    ) -> Result<Response<ChangeHandleResponse>, Status> {
        let proto_req = request.get_ref();

        let target = proto_req
            .target
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Missing target"))?;
        let profile_id: crate::value_objects::ProfileId = target
            .profile_id
            .parse()
            .map_err(|e| Status::invalid_argument(format!("Invalid profile_id: {}", e)))?;

        let ctx = self.get_context(&request, &profile_id)?;

        let req = request.into_inner();
        let command = ChangeHandleCommand::try_from_proto(req)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        self.execute_and_fetch::<ChangeHandleCommand, (), ChangeHandleResponse, _>(
            &ctx,
            command,
            |profile| ChangeHandleResponse {
                profile: Some(map_profile_to_proto(profile)),
            },
        )
        .await
    }

    async fn update_privacy(
        &self,
        request: Request<UpdatePrivacyRequest>,
    ) -> Result<Response<UpdatePrivacyResponse>, Status> {
        let proto_req = request.get_ref();

        let target = proto_req
            .target
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Missing target"))?;
        let profile_id = target
            .profile_id
            .parse()
            .map_err(|e| Status::invalid_argument(format!("Invalid profile_id: {}", e)))?;

        let ctx = self.get_context(&request, &profile_id)?;

        let req = request.into_inner();
        let command = UpdatePrivacyCommand::try_from_proto(req)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        self.execute_and_fetch::<UpdatePrivacyCommand, (), UpdatePrivacyResponse, _>(
            &ctx,
            command,
            |profile| UpdatePrivacyResponse {
                profile: Some(map_profile_to_proto(profile)),
            },
        )
        .await
    }
}
