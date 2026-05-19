use std::sync::Arc;
use tonic::{Request, Response, Status};

// Kernel & Shared Proto Imports
use shared_kernel::command::CommandBus;
use shared_kernel::types::{ProfileId, Region};
use shared_proto::social::v1::social_service_server::SocialService as ProtoSocialService;
use shared_proto::social::v1::{
    FollowProfileRequest, FollowProfileResponse, GetFollowersRequest, GetFollowersResponse,
    GetFollowingRequest, GetFollowingResponse, GetProfileCountersRequest,
    GetProfileCountersResponse, IsFollowingRequest, IsFollowingResponse, UnfollowProfileRequest,
    UnfollowProfileResponse,
};

// Application & Context Imports
use crate::commands::{FollowCommand, UnfollowCommand};
use crate::context::SocialAppContext;
use crate::utils::{GrpcServiceUtils, map_domain_err_to_status};

pub struct SocialService {
    bus: Arc<CommandBus>,
    app_ctx: Arc<SocialAppContext>,
}

impl SocialService {
    pub fn new(bus: Arc<CommandBus>, app_ctx: Arc<SocialAppContext>) -> Self {
        Self { bus, app_ctx }
    }
}

impl GrpcServiceUtils for SocialService {
    type AppContext = SocialAppContext;

    fn app_ctx(&self) -> &SocialAppContext {
        &self.app_ctx
    }
    fn bus(&self) -> &CommandBus {
        &self.bus
    }
}

#[tonic::async_trait]
impl ProtoSocialService for SocialService {
    // --- WRITES ---
    async fn follow_profile(
        &self,
        request: Request<FollowProfileRequest>,
    ) -> Result<Response<FollowProfileResponse>, Status> {
        let (_metadata, extensions, req_inner) = request.into_parts();

        let target = req_inner
            .target
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Missing target context"))?;

        let target_profile_id = target
            .profile_id
            .parse::<ProfileId>()
            .map_err(|e| Status::invalid_argument(format!("Invalid target profile_id: {}", e)))?;

        let ctx = self.build_context(target_profile_id, &extensions)?;

        // On map le proto sur ta commande interne
        let command = FollowCommand::try_from_proto(req_inner)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        self.dispatch_command::<FollowCommand, (), FollowProfileResponse>(
            &ctx,
            command,
            FollowProfileResponse { success: true },
        )
        .await
    }

    async fn unfollow_profile(
        &self,
        request: Request<UnfollowProfileRequest>,
    ) -> Result<Response<UnfollowProfileResponse>, Status> {
        let (_metadata, extensions, req_inner) = request.into_parts();

        let target = req_inner
            .target
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Missing target context"))?;

        let target_profile_id = target
            .profile_id
            .parse::<ProfileId>()
            .map_err(|e| Status::invalid_argument(format!("Invalid target profile_id: {}", e)))?;

        let ctx = self.build_context(target_profile_id, &extensions)?;

        let command = UnfollowCommand::try_from_proto(req_inner)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        self.dispatch_command::<UnfollowCommand, (), UnfollowProfileResponse>(
            &ctx,
            command,
            UnfollowProfileResponse { success: true },
        )
        .await
    }

    // --- READS ---

    // AJOUT : Implémentation de la vérification de relation
    async fn is_following(
        &self,
        request: Request<IsFollowingRequest>,
    ) -> Result<Response<IsFollowingResponse>, Status> {
        let req = request.into_inner();

        let follower_id = req
            .follower_id
            .parse::<ProfileId>()
            .map_err(|e| Status::invalid_argument(format!("Invalid follower_id: {}", e)))?;

        let following_id = req
            .following_id
            .parse::<ProfileId>()
            .map_err(|e| Status::invalid_argument(format!("Invalid following_id: {}", e)))?;

        let is_following = self
            .app_ctx
            .relation_repo()
            .is_following(follower_id, following_id)
            .await
            .map_err(map_domain_err_to_status)?;

        Ok(Response::new(IsFollowingResponse { is_following }))
    }

    async fn get_profile_counters(
        &self,
        request: Request<GetProfileCountersRequest>,
    ) -> Result<Response<GetProfileCountersResponse>, Status> {
        let req = request.into_inner();

        let profile_id = req
            .profile_id
            .parse::<ProfileId>()
            .map_err(|e| Status::invalid_argument(format!("Invalid profile_id format: {}", e)))?;

        let region = req
            .region
            .parse::<Region>()
            .map_err(|e| Status::invalid_argument(format!("Invalid region format: {}", e)))?;

        let context = self.app_ctx.create_context(profile_id, region);

        let counters = context
            .get_profile_counters(profile_id)
            .await
            .map_err(map_domain_err_to_status)?;

        Ok(Response::new(GetProfileCountersResponse {
            followers_count: counters.followers_count().value(),
            following_count: counters.following_count().value(),
        }))
    }

    async fn get_following(
        &self,
        request: Request<GetFollowingRequest>,
    ) -> Result<Response<GetFollowingResponse>, Status> {
        let req = request.into_inner();

        let follower_id = req
            .follower_id
            .parse::<ProfileId>()
            .map_err(|e| Status::invalid_argument(format!("Invalid follower_id format: {}", e)))?;

        let region = req
            .region
            .parse::<Region>()
            .map_err(|e| Status::invalid_argument(format!("Invalid region format: {}", e)))?;

        let context = self.app_ctx.create_context(follower_id, region);

        let ids = context
            .get_following_list(
                follower_id,
                req.limit.unwrap_or(20),
                req.offset.unwrap_or(0),
            )
            .await
            .map_err(map_domain_err_to_status)?;

        let string_ids = ids.into_iter().map(|id| id.to_string()).collect();

        Ok(Response::new(GetFollowingResponse {
            following_ids: string_ids,
        }))
    }

    async fn get_followers(
        &self,
        request: Request<GetFollowersRequest>,
    ) -> Result<Response<GetFollowersResponse>, Status> {
        let req = request.into_inner();

        let following_id = req
            .following_id
            .parse::<ProfileId>()
            .map_err(|e| Status::invalid_argument(format!("Invalid following_id format: {}", e)))?;

        let region = req
            .region
            .parse::<Region>()
            .map_err(|e| Status::invalid_argument(format!("Invalid region format: {}", e)))?;

        let context = self.app_ctx.create_context(following_id, region);

        let ids = context
            .get_followers_list(
                following_id,
                req.limit.unwrap_or(20),
                req.offset.unwrap_or(0),
            )
            .await
            .map_err(map_domain_err_to_status)?;

        let string_ids = ids.into_iter().map(|id| id.to_string()).collect();

        Ok(Response::new(GetFollowersResponse {
            followers_ids: string_ids,
        }))
    }
}
