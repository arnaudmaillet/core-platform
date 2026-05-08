// crates/profile/src/presentation/grpc/profile_service.rs

use tonic::{Request, Response, Status};
use shared_proto::profile::v1::profile_service_server::ProfileService;
use shared_proto::profile::v1::{GetProfileRequest, ProfileResponse};
use crate::application::ProfileContext;
use crate::domain::value_objects::ProfileId;
use shared_kernel::domain::value_objects::RegionCode;

pub struct GrpcProfileService {
    // On injecte ce qu'il faut pour créer des contextes
    // ou on injecte directement un orchestrateur
}

#[tonic::async_trait]
impl ProfileService for GrpcProfileService {
    async fn get_profile(
        &self,
        request: Request<GetProfileRequest>,
    ) -> Result<Response<ProfileResponse>, Status> {
        let req = request.into_inner();

        // 1. Mapping Entrant (Proto -> Domain)
        let profile_id = ProfileId::try_from(req.profile_id)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;
        let region = RegionCode::from_str(&req.region)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        // 2. Initialisation du contexte (via ton Builder)
        // Ici, on imagine que tu as un service qui te donne le contexte
        let ctx = self.app_factory.create_context(profile_id, region);

        // 3. Appel métier
        let profile = ctx.profile(None).await
            .map_err(|e| match e {
                DomainError::NotFound { .. } => Status::not_found("Profile not found"),
                _ => Status::internal("Internal error"),
            })?;

        // 4. Mapping Sortant (Domain -> Proto)
        Ok(Response::new(ProfileResponse::from_domain(profile)))
    }
}