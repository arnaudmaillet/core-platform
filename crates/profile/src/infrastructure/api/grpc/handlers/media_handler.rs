// crates/profile/src/infrastructure/api/grpc/handlers/media_handler.rs

use std::sync::Arc;
use tonic::{Request, Response, Status};

// Valeurs du domaine
use crate::application::remove_avatar::RemoveAvatarUseCase;
use crate::application::remove_banner::RemoveBannerUseCase;
use crate::application::update_avatar::UpdateAvatarUseCase;
use crate::application::update_banner::UpdateBannerUseCase;
use shared_kernel::domain::value_objects::{AccountId, RegionCode, Url};

// Commandes
use crate::application::remove_avatar::RemoveAvatarCommand;
use crate::application::remove_banner::RemoveBannerCommand;
use crate::application::update_avatar::UpdateAvatarCommand;
use crate::application::update_banner::UpdateBannerCommand;

// Proto généré
use super::super::profile_v1::{
    Profile as ProtoProfile, RemoveAvatarRequest, RemoveBannerRequest, UpdateAvatarRequest,
    UpdateBannerRequest, profile_media_service_server::ProfileMediaService,
};

pub struct MediaHandler {
    update_avatar_uc: Arc<UpdateAvatarUseCase>,
    remove_avatar_uc: Arc<RemoveAvatarUseCase>,
    update_banner_uc: Arc<UpdateBannerUseCase>,
    remove_banner_uc: Arc<RemoveBannerUseCase>,
}

impl MediaHandler {
    pub fn new(
        update_avatar_uc: Arc<UpdateAvatarUseCase>,
        remove_avatar_uc: Arc<RemoveAvatarUseCase>,
        update_banner_uc: Arc<UpdateBannerUseCase>,
        remove_banner_uc: Arc<RemoveBannerUseCase>,
    ) -> Self {
        Self {
            update_avatar_uc,
            remove_avatar_uc,
            update_banner_uc,
            remove_banner_uc,
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
impl ProfileMediaService for MediaHandler {
    // --- AVATAR ---

    async fn update_avatar(
        &self,
        request: Request<UpdateAvatarRequest>,
    ) -> Result<Response<ProtoProfile>, Status> {
        let region = self.get_region(&request)?;
        let req = request.into_inner();

        let account_id = AccountId::try_from(req.account_id)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;
        let new_avatar_url = Url::try_from(req.new_avatar_url)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        let command = UpdateAvatarCommand {
            account_id,
            region,
            new_avatar_url,
        };

        let profile = self
            .update_avatar_uc
            .execute(command)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(profile.into()))
    }

    async fn remove_avatar(
        &self,
        request: Request<RemoveAvatarRequest>,
    ) -> Result<Response<ProtoProfile>, Status> {
        let region = self.get_region(&request)?;
        let req = request.into_inner();

        let account_id = AccountId::try_from(req.account_id)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        let command = RemoveAvatarCommand { account_id, region };

        let profile = self
            .remove_avatar_uc
            .execute(command)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(profile.into()))
    }

    // --- BANNER ---

    async fn update_banner(
        &self,
        request: Request<UpdateBannerRequest>,
    ) -> Result<Response<ProtoProfile>, Status> {
        let region = self.get_region(&request)?;
        let req = request.into_inner();

        let account_id = AccountId::try_from(req.account_id)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;
        let new_banner_url = Url::try_from(req.new_banner_url)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        let command = UpdateBannerCommand {
            account_id,
            region,
            new_banner_url,
        };

        let profile = self
            .update_banner_uc
            .execute(command)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(profile.into()))
    }

    async fn remove_banner(
        &self,
        request: Request<RemoveBannerRequest>,
    ) -> Result<Response<ProtoProfile>, Status> {
        let region = self.get_region(&request)?;
        let req = request.into_inner();

        let account_id = AccountId::try_from(req.account_id)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        let command = RemoveBannerCommand { account_id, region };

        let profile = self
            .remove_banner_uc
            .execute(command)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(profile.into()))
    }
}
