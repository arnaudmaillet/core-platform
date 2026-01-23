// crates/profile/src/infrastructure/api/grpc/handlers/counter_handler.rs

use tonic::{Request, Response, Status};
use shared_kernel::domain::value_objects::{AccountId, PostId, RegionCode};
use crate::infrastructure::api::grpc::profile_v1::{
    profile_counter_service_server::ProfileCounterService,
    IncrementPostCountRequest,
    DecrementPostCountRequest,
    Profile as ProtoProfile
};
use crate::application::{
    increment_post_count::IncrementPostCountCommand,
    decrement_post_count::DecrementPostCountCommand,
    increment_post_count::IncrementPostCountUseCase,
    decrement_post_count::DecrementPostCountUseCase,
};
use std::sync::Arc;

pub struct ProfileCounterHandler {
    increment_post_uc: Arc<IncrementPostCountUseCase>,
    decrement_post_uc: Arc<DecrementPostCountUseCase>,
}

impl ProfileCounterHandler {
    pub fn new(
        increment_post_uc: Arc<IncrementPostCountUseCase>,
        decrement_post_uc: Arc<DecrementPostCountUseCase>,
    ) -> Self {
        Self {
            increment_post_uc,
            decrement_post_uc,
        }
    }

    /// Helper pour extraire la région injectée par l'intercepteur
    fn get_region<T>(&self, request: &Request<T>) -> Result<RegionCode, Status> {
        request.extensions()
            .get::<RegionCode>()
            .cloned()
            .ok_or_else(|| Status::internal("Region context missing from metadata"))
    }
}

#[tonic::async_trait]
impl ProfileCounterService for ProfileCounterHandler {
    /// Incrémente le compteur de posts
    async fn increment_post_count(
        &self,
        request: Request<IncrementPostCountRequest>,
    ) -> Result<Response<ProtoProfile>, Status> {
        
        let region = self.get_region(&request)?;
        let req = request.into_inner();
        
        // Conversion et validation de l'ID
        let account_id = AccountId::try_from(req.account_id)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;
        
        let post_id = PostId::try_from(req.post_id)
        .map_err(|e| Status::invalid_argument(e.to_string()))?;

        // Création de la commande
        let command = IncrementPostCountCommand { account_id, region, post_id };

        // Exécution du Use Case
        let profile = self.increment_post_uc
            .execute(command)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        // Transformation de l'entité Domaine en Proto via notre Mapper
        Ok(Response::new(profile.into()))
    }

    /// Décrémente le compteur de posts
    async fn decrement_post_count(
        &self,
        request: Request<DecrementPostCountRequest>,
    ) -> Result<Response<ProtoProfile>, Status> {
        let region = self.get_region(&request)?;
        let req = request.into_inner();

        let post_id = PostId::try_from(req.post_id)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        let account_id = AccountId::try_from(req.account_id)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        let command = DecrementPostCountCommand { account_id, region, post_id };

        let profile = self.decrement_post_uc
            .execute(command)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(profile.into()))
    }
}