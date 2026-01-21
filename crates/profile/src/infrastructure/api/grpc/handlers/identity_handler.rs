// crates/profile/src/infrastructure/api/grpc/handlers/identity_handler.rs

use std::sync::Arc;
use tonic::{Request, Response, Status};
use shared_kernel::domain::value_objects::{RegionCode, Username, AccountId};

use crate::application::update_username::{UpdateUsernameUseCase, UpdateUsernameCommand};
use super::super::profile_v1::{
    profile_identity_service_server::ProfileIdentityService,
    UpdateUsernameRequest,
    Profile as ProtoProfile,
};

pub struct IdentityHandler {
    use_case: Arc<UpdateUsernameUseCase>
}

impl IdentityHandler {
    pub fn new(use_case: Arc<UpdateUsernameUseCase>) -> Self {
        Self { use_case }
    }
}

#[tonic::async_trait]
impl ProfileIdentityService for IdentityHandler {
    async fn update_username(
        &self,
        request: Request<UpdateUsernameRequest>,
    ) -> Result<Response<ProtoProfile>, Status> {

        let region = request.extensions()
            .get::<RegionCode>()
            .cloned()
            .ok_or_else(|| Status::internal("Region context missing from interceptor"))?;

        let req = request.into_inner();

        // 1. Transformation des types Proto vers Value Objects du Domaine
        // C'est ici que l'on valide le format avant d'entrer dans le Use Case
        let account_id = AccountId::try_from(req.account_id)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        let new_username = Username::try_from(req.new_username)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        // 2. Construction de la Command (Pattern Hyperscale)
        let command = UpdateUsernameCommand {
            account_id: account_id.clone(),
            region: region.clone(),
            new_username,
        };

        let profile = self.use_case
            .execute(command)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(profile.into()))
    }
}