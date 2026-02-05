// crates/profile/src/infrastructure/api/grpc/handlers/identity_handler.rs

use super::super::profile_v1::{
    Profile as ProtoProfile, UpdateDisplayNameRequest, UpdatePrivacyRequest, UpdateUsernameRequest,
    profile_identity_service_server::ProfileIdentityService,
};
use crate::application::update_display_name::{UpdateDisplayNameCommand, UpdateDisplayNameUseCase};
use crate::application::update_privacy::{UpdatePrivacyCommand, UpdatePrivacyUseCase};
use crate::application::update_username::{UpdateUsernameCommand, UpdateUsernameUseCase};

use shared_kernel::domain::value_objects::RegionCode;
use std::sync::Arc;
use tonic::{Request, Response, Status};

pub struct IdentityHandler {
    update_username_use_case: Arc<UpdateUsernameUseCase>,
    update_display_name_use_case: Arc<UpdateDisplayNameUseCase>,
    update_privacy_use_case: Arc<UpdatePrivacyUseCase>,
}

impl IdentityHandler {
    pub fn new(
        update_username_use_case: Arc<UpdateUsernameUseCase>,
        update_display_name_use_case: Arc<UpdateDisplayNameUseCase>,
        update_privacy_use_case: Arc<UpdatePrivacyUseCase>,
    ) -> Self {
        Self {
            update_username_use_case,
            update_display_name_use_case,
            update_privacy_use_case,
        }
    }

    fn get_region<T>(&self, request: &Request<T>) -> Result<RegionCode, Status> {
        request
            .extensions()
            .get::<RegionCode>()
            .cloned()
            .ok_or_else(|| Status::internal("Region context missing from metadata"))
    }
}

#[tonic::async_trait]
impl ProfileIdentityService for IdentityHandler {
    async fn update_username(&self, request: Request<UpdateUsernameRequest>) -> Result<Response<ProtoProfile>, Status> {
        let region = self.get_region(&request)?;
        let command = UpdateUsernameCommand::try_from_proto(request.into_inner(), region)?;

        let profile = self.update_username_use_case.execute(command).await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(profile.into()))
    }

    async fn update_display_name(&self, request: Request<UpdateDisplayNameRequest>) -> Result<Response<ProtoProfile>, Status> {
        let region = self.get_region(&request)?;
        let command = UpdateDisplayNameCommand::try_from_proto(request.into_inner(), region)?;

        let profile = self.update_display_name_use_case.execute(command).await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(profile.into()))
    }

    async fn update_privacy(&self, request: Request<UpdatePrivacyRequest>) -> Result<Response<ProtoProfile>, Status> {
        let region = self.get_region(&request)?;
        let command = UpdatePrivacyCommand::try_from_proto(request.into_inner(), region)?;

        let profile = self.update_privacy_use_case.execute(command).await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(profile.into()))
    }
}
