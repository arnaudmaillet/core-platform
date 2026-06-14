// crates/social/src/presentation/services/social.rs

use crate::application::context::SocialKernelCtx;
use crate::domain::repositories::ProfileCountersStorageRepository;
use crate::use_cases::{FollowCommand, UnfollowCommand};
use crate::utils::{GrpcServiceUtils, map_domain_err_to_status};
use shared_kernel::command::CommandBus;
use shared_kernel::types::ProfileId;
use shared_proto::social::v1::social_service_server::SocialService as ProtoSocialService;
use shared_proto::social::v1::{
    FollowProfileRequest, FollowProfileResponse, GetFollowersRequest, GetFollowersResponse,
    GetFollowingRequest, GetFollowingResponse, GetProfileCountersRequest,
    GetProfileCountersResponse, IsFollowingRequest, IsFollowingResponse, UnfollowProfileRequest,
    UnfollowProfileResponse,
};
use std::sync::Arc;
use tonic::{Request, Response, Status};

pub struct SocialService {
    bus: CommandBus,
    kernel: SocialKernelCtx,
    profile_counters_storage: Arc<dyn ProfileCountersStorageRepository>,
}

impl SocialService {
    pub fn new(
        bus: CommandBus,
        kernel: SocialKernelCtx,
        profile_counters_storage: Arc<dyn ProfileCountersStorageRepository>,
    ) -> Self {
        Self {
            bus,
            kernel,
            profile_counters_storage,
        }
    }
}

impl GrpcServiceUtils for SocialService {
    type AppContext = SocialKernelCtx;

    fn kernel(&self) -> &SocialKernelCtx {
        &self.kernel
    }

    fn bus(&self) -> &CommandBus {
        &self.bus
    }

    fn profile_counters_storage(&self) -> &Arc<dyn ProfileCountersStorageRepository> {
        &self.profile_counters_storage
    }
}

#[tonic::async_trait]
impl ProtoSocialService for SocialService {
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
        let ctx = self.build_command_ctx(target_profile_id, &extensions)?;

        let command = FollowCommand::try_from_proto(req_inner, ctx.region())
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

        let ctx = self.build_command_ctx(target_profile_id, &extensions)?;

        let command = UnfollowCommand::try_from_proto(req_inner, ctx.region())
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        self.dispatch_command::<UnfollowCommand, (), UnfollowProfileResponse>(
            &ctx,
            command,
            UnfollowProfileResponse { success: true },
        )
        .await
    }

    async fn is_following(
        &self,
        request: Request<IsFollowingRequest>,
    ) -> Result<Response<IsFollowingResponse>, Status> {
        let (_metadata, extensions, req) = request.into_parts();

        let follower_id = req
            .follower_id
            .parse::<ProfileId>()
            .map_err(|e| Status::invalid_argument(format!("Invalid follower_id: {}", e)))?;

        let following_id = req
            .following_id
            .parse::<ProfileId>()
            .map_err(|e| Status::invalid_argument(format!("Invalid following_id: {}", e)))?;

        let query_ctx = self.build_query_ctx(&extensions)?;

        let is_following = query_ctx
            .is_already_following(follower_id, following_id)
            .await
            .map_err(map_domain_err_to_status)?;

        Ok(Response::new(IsFollowingResponse { is_following }))
    }

    async fn get_profile_counters(
        &self,
        request: Request<GetProfileCountersRequest>,
    ) -> Result<Response<GetProfileCountersResponse>, Status> {
        let (_metadata, extensions, req) = request.into_parts();

        let profile_id = req
            .profile_id
            .parse::<ProfileId>()
            .map_err(|e| Status::invalid_argument(format!("Invalid profile_id format: {}", e)))?;

        let query_ctx = self.build_query_ctx(&extensions)?;

        let counters = query_ctx
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
        let (_metadata, extensions, req) = request.into_parts();

        let follower_id = req
            .follower_id
            .parse::<ProfileId>()
            .map_err(|e| Status::invalid_argument(format!("Invalid follower_id format: {}", e)))?;

        let query_ctx = self.build_query_ctx(&extensions)?;

        let ids = query_ctx
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
        let (_metadata, extensions, req) = request.into_parts();

        let following_id = req
            .following_id
            .parse::<ProfileId>()
            .map_err(|e| Status::invalid_argument(format!("Invalid following_id format: {}", e)))?;

        let query_ctx = self.build_query_ctx(&extensions)?;

        let ids = query_ctx
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
