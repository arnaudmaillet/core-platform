use tonic::{Request, Response, Status};

use cqrs::{CommandBus, QueryBus};

use super::handler::social_graph_service_handler::{proto, SocialGraphServiceHandler};
use proto::social_graph_service_server::SocialGraphService;

#[tonic::async_trait]
impl<CB, QB> SocialGraphService for SocialGraphServiceHandler<CB, QB>
where
    CB: CommandBus + Send + Sync + 'static,
    QB: QueryBus + Send + Sync + 'static,
{
    // ── Commands ──────────────────────────────────────────────────────────────

    async fn follow(
        &self,
        request: Request<proto::FollowRequest>,
    ) -> Result<Response<proto::CommandResponse>, Status> {
        self.follow(request).await
    }

    async fn unfollow(
        &self,
        request: Request<proto::UnfollowRequest>,
    ) -> Result<Response<proto::CommandResponse>, Status> {
        self.unfollow(request).await
    }

    async fn block(
        &self,
        request: Request<proto::BlockRequest>,
    ) -> Result<Response<proto::CommandResponse>, Status> {
        self.block(request).await
    }

    async fn unblock(
        &self,
        request: Request<proto::UnblockRequest>,
    ) -> Result<Response<proto::CommandResponse>, Status> {
        self.unblock(request).await
    }

    // ── Queries ───────────────────────────────────────────────────────────────

    async fn get_relation_status(
        &self,
        request: Request<proto::GetRelationStatusRequest>,
    ) -> Result<Response<proto::RelationStatusView>, Status> {
        self.get_relation_status(request).await
    }

    async fn list_followers(
        &self,
        request: Request<proto::ListFollowersRequest>,
    ) -> Result<Response<proto::ListFollowersResponse>, Status> {
        self.list_followers(request).await
    }

    async fn list_following(
        &self,
        request: Request<proto::ListFollowingRequest>,
    ) -> Result<Response<proto::ListFollowingResponse>, Status> {
        self.list_following(request).await
    }

    async fn list_blocks(
        &self,
        request: Request<proto::ListBlocksRequest>,
    ) -> Result<Response<proto::ListBlocksResponse>, Status> {
        self.list_blocks(request).await
    }
}
