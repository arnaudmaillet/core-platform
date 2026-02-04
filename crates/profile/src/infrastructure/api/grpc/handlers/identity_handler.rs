// crates/profile/src/infrastructure/api/grpc/handlers/identity_handler.rs

use super::super::profile_v1::{
    Profile as ProtoProfile, UpdateDisplayNameRequest, UpdatePrivacyRequest, UpdateUsernameRequest,
    profile_identity_service_server::ProfileIdentityService,
};
use crate::application::update_display_name::{UpdateDisplayNameCommand, UpdateDisplayNameUseCase};
use crate::application::update_privacy::{UpdatePrivacyCommand, UpdatePrivacyUseCase};
use crate::application::update_username::{UpdateUsernameCommand, UpdateUsernameUseCase};
use crate::domain::value_objects::DisplayName;
use shared_kernel::domain::value_objects::{AccountId, RegionCode, Username};
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
    async fn update_username(
        &self,
        request: Request<UpdateUsernameRequest>,
    ) -> Result<Response<ProtoProfile>, Status> {
        let region = self.get_region(&request)?;
        let req = request.into_inner();

        // 1. Transformation des types Proto vers Value Objects du Domaine
        let account_id = AccountId::try_from(req.account_id)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        let new_username = Username::try_from(req.new_username)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        // 2. Construction de la Command (Pattern Hyperscale)
        let command = UpdateUsernameCommand {
            account_id,
            region,
            new_username,
        };

        let profile = self
            .update_username_use_case
            .execute(command)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(profile.into()))
    }

    async fn update_display_name(
        &self,
        request: Request<UpdateDisplayNameRequest>,
    ) -> Result<Response<ProtoProfile>, Status> {
        let region = self.get_region(&request)?;
        let req = request.into_inner();

        // 1. Transformation des types Proto vers Value Objects du Domaine
        let account_id = AccountId::try_from(req.account_id)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        let new_display_name = DisplayName::try_from(req.new_display_name)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        // 2. Construction de la Command
        let command = UpdateDisplayNameCommand {
            account_id,
            region,
            new_display_name,
        };

        let profile = self
            .update_display_name_use_case
            .execute(command)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(profile.into()))
    }

    async fn update_privacy(
        &self,
        request: Request<UpdatePrivacyRequest>,
    ) -> Result<Response<ProtoProfile>, Status> {
        let region = self.get_region(&request)?;
        let req = request.into_inner();
        let is_private = req.is_private;

        // 1. Transformation des types Proto vers Value Objects du Domaine
        let account_id = AccountId::try_from(req.account_id)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        // 2. Construction de la Command
        let command = UpdatePrivacyCommand {
            account_id,
            region,
            is_private,
        };

        let profile = self
            .update_privacy_use_case
            .execute(command)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(profile.into()))
    }
}
