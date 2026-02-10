// crates/profile/src/infrastructure/api/grpc/handlers/counter_handler.rs

use crate::application::{
    decrement_post_count::DecrementPostCountCommand,
    decrement_post_count::DecrementPostCountUseCase,
    increment_post_count::IncrementPostCountCommand,
    increment_post_count::IncrementPostCountUseCase,
};
use crate::infrastructure::api::grpc::profile_v1::{
    DecrementPostCountRequest, IncrementPostCountRequest, Profile as ProtoProfile,
    profile_counter_service_server::ProfileCounterService,
};
use shared_kernel::domain::value_objects::{AccountId, PostId, RegionCode};
use std::sync::Arc;
use tonic::{Request, Response, Status};
use crate::infrastructure::api::grpc::mappers::ToGrpcStatus;

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
        request
            .extensions()
            .get::<RegionCode>()
            .cloned()
            .ok_or_else(|| Status::internal("Region context missing from metadata"))
    }
}

#[tonic::async_trait]
impl ProfileCounterService for ProfileCounterHandler {
    async fn increment_post_count(
        &self,
        request: Request<IncrementPostCountRequest>,
    ) -> Result<Response<ProtoProfile>, Status> {
        let region = self.get_region(&request)?;
        let command = IncrementPostCountCommand::try_from_proto(request.into_inner(), region)?;
        let profile = self.increment_post_uc.execute(command).await.map_grpc()?;
        Ok(Response::new(profile.into()))
    }

    async fn decrement_post_count(
        &self,
        request: Request<DecrementPostCountRequest>,
    ) -> Result<Response<ProtoProfile>, Status> {
        let region = self.get_region(&request)?;
        let command = DecrementPostCountCommand::try_from_proto(request.into_inner(), region)?;
        let profile = self.decrement_post_uc.execute(command).await.map_grpc()?;
        Ok(Response::new(profile.into()))
    }
}