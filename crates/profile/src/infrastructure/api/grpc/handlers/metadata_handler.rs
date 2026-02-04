// crates/profile/src/infrastructure/api/grpc/handlers/metadata_handler.rs

use std::sync::Arc;
use tonic::{Request, Response, Status};

// Valeurs du domaine et Use Cases
use crate::application::update_bio::{UpdateBioCommand, UpdateBioUseCase};
use crate::application::update_location_label::{
    UpdateLocationLabelCommand, UpdateLocationLabelUseCase,
};
use crate::application::update_social_links::{UpdateSocialLinksCommand, UpdateSocialLinksUseCase};
use crate::domain::value_objects::{Bio, SocialLinks};
use shared_kernel::domain::value_objects::{AccountId, LocationLabel, RegionCode};
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
    async fn update_bio(
        &self,
        request: Request<UpdateBioRequest>,
    ) -> Result<Response<ProtoProfile>, Status> {
        let region = self.get_region(&request)?;
        let req = request.into_inner();

        let account_id = AccountId::try_from(req.account_id)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        let new_bio = req
            .new_bio
            .filter(|s| !s.trim().is_empty())
            .map(|s| Bio::try_from(s).map_err(|e| Status::invalid_argument(e.to_string())))
            .transpose()?;

        let command = UpdateBioCommand {
            account_id,
            region,
            new_bio,
        };

        let profile = self
            .update_bio_uc
            .execute(command)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(profile.into()))
    }

    async fn update_location_label(
        &self,
        request: Request<UpdateLocationLabelRequest>,
    ) -> Result<Response<ProtoProfile>, Status> {
        let region = self.get_region(&request)?;
        let req = request.into_inner();

        let new_location = req
            .new_location_label
            .filter(|s| !s.trim().is_empty())
            .map(|s| {
                LocationLabel::try_from(s).map_err(|e| Status::invalid_argument(e.to_string()))
            })
            .transpose()?;

        let account_id = AccountId::try_from(req.account_id)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        let command = UpdateLocationLabelCommand {
            account_id,
            region,
            new_location,
        };

        let profile = self
            .update_location_uc
            .execute(command)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(profile.into()))
    }

    async fn update_social_links(
        &self,
        request: Request<UpdateSocialLinksRequest>,
    ) -> Result<Response<ProtoProfile>, Status> {
        let region = self.get_region(&request)?;
        let req = request.into_inner();
        let new_links = req
            .new_links
            .map(|l| SocialLinks::try_from(l).map_err(|e| Status::invalid_argument(e.to_string())))
            .transpose()?;

        let account_id = AccountId::try_from(req.account_id)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        let command = UpdateSocialLinksCommand {
            account_id,
            region,
            new_links,
        };

        let profile = self
            .update_social_links_uc
            .execute(command)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(profile.into()))
    }
}
