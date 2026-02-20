// crates/profile/src/infrastructure/api/grpc/handlers/metadata_handler.rs

use std::sync::Arc;
use tonic::{Request, Response, Status};

// Valeurs du domaine et Use Cases
use crate::application::use_cases::update_bio::{UpdateBioCommand, UpdateBioUseCase};
use crate::application::use_cases::update_location_label::{
    UpdateLocationLabelCommand, UpdateLocationLabelUseCase,
};
use crate::application::use_cases::update_social_links::{UpdateSocialLinksCommand, UpdateSocialLinksUseCase};
use shared_kernel::domain::value_objects::RegionCode;
use crate::infrastructure::api::grpc::mappers::ToGrpcStatus;
// Proto généré
use super::super::profile_v1::{
    Profile as ProtoProfile, UpdateBioRequest, UpdateLocationLabelRequest,
    UpdateSocialLinksRequest, profile_metadata_service_server::ProfileMetadataService,
};

pub struct MetadataHandler {
    update_bio_uc: Arc<UpdateBioUseCase>,
    update_location_uc: Arc<UpdateLocationLabelUseCase>,
    update_social_links_uc: Arc<UpdateSocialLinksUseCase>,
}

impl MetadataHandler {
    pub fn new(
        update_bio_uc: Arc<UpdateBioUseCase>,
        update_location_uc: Arc<UpdateLocationLabelUseCase>,
        update_social_links_uc: Arc<UpdateSocialLinksUseCase>,
    ) -> Self {
        Self {
            update_bio_uc,
            update_location_uc,
            update_social_links_uc,
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
impl ProfileMetadataService for MetadataHandler {
    async fn update_bio(&self, request: Request<UpdateBioRequest>) -> Result<Response<ProtoProfile>, Status> {
        let region = self.get_region(&request)?;
        let command = UpdateBioCommand::try_from_proto(request.into_inner(), region)?;
        let profile = self.update_bio_uc.execute(command).await.map_grpc()?;
        Ok(Response::new(profile.into()))
    }

    async fn update_location_label(&self, request: Request<UpdateLocationLabelRequest>) -> Result<Response<ProtoProfile>, Status> {
        let region = self.get_region(&request)?;
        let command = UpdateLocationLabelCommand::try_from_proto(request.into_inner(), region)?;
        let profile = self.update_location_uc.execute(command).await.map_grpc()?;
        Ok(Response::new(profile.into()))
    }

    async fn update_social_links(&self, request: Request<UpdateSocialLinksRequest>) -> Result<Response<ProtoProfile>, Status> {
        let region = self.get_region(&request)?;
        let command = UpdateSocialLinksCommand::try_from_proto(request.into_inner(), region)?;
        let profile = self.update_social_links_uc.execute(command).await.map_grpc()?;
        Ok(Response::new(profile.into()))
    }
}
