// crates/content_comments/src/presentation/grpc/comment_service.rs

use std::sync::Arc;
use tonic::{Request, Response, Status};
use uuid::Uuid;

use shared_kernel::command::CommandBus;
use shared_kernel::core::PageQuery;
use shared_kernel::types::{PostId, ProfileId};
use shared_proto::comment::v1::comment_service_server::CommentService as ProtoCommentService;
use shared_proto::comment::v1::*;

use crate::application::commands::{
    DeleteCommentCommand, EditCommentContentCommand, PublishCommentCommand,
};
use crate::application::context::CommentAppContext;
use crate::types::CommentId;
use crate::utils::{GrpcServiceUtils, map_domain_err_to_status};

pub struct CommentService {
    bus: Arc<CommandBus>,
    app_ctx: Arc<CommentAppContext>,
}

impl CommentService {
    pub fn new(bus: Arc<CommandBus>, app_ctx: Arc<CommentAppContext>) -> Self {
        Self { bus, app_ctx }
    }
}

impl GrpcServiceUtils for CommentService {
    type AppContext = CommentAppContext;

    fn app_ctx(&self) -> &CommentAppContext {
        &self.app_ctx
    }
    fn bus(&self) -> &CommandBus {
        &self.bus
    }
}

#[tonic::async_trait]
impl ProtoCommentService for CommentService {
    async fn publish_comment(
        &self,
        request: Request<PublishCommentRequest>,
    ) -> Result<Response<PublishCommentResponse>, Status> {
        let (_meta, _ext, req) = request.into_parts();

        let profile_id = ProfileId::try_from(req.profile_id.clone())
            .map_err(|e| Status::invalid_argument(format!("Invalid profile_id: {}", e)))?;

        let ctx = self.app_ctx.command(profile_id);

        let generated_comment_id = CommentId::from(Uuid::now_v7());

        let command = PublishCommentCommand::try_from_proto(req)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        self.dispatch_command::<PublishCommentCommand, (), PublishCommentResponse>(
            &ctx,
            command,
            PublishCommentResponse {
                comment_id: generated_comment_id.to_string(),
            },
        )
        .await
    }

    async fn edit_comment_content(
        &self,
        request: Request<EditCommentContentRequest>,
    ) -> Result<Response<EditCommentContentResponse>, Status> {
        let (_meta, _ext, req) = request.into_parts();

        let operator_id = ProfileId::try_from(req.operator_id.clone())
            .map_err(|e| Status::invalid_argument(format!("Invalid operator_id: {}", e)))?;

        let ctx = self.app_ctx.command(operator_id);

        let command = EditCommentContentCommand::try_from_proto(req)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        self.dispatch_command::<EditCommentContentCommand, (), EditCommentContentResponse>(
            &ctx,
            command,
            EditCommentContentResponse {},
        )
        .await
    }

    async fn delete_comment(
        &self,
        request: Request<DeleteCommentRequest>,
    ) -> Result<Response<DeleteCommentResponse>, Status> {
        let (_meta, ext, req) = request.into_parts();

        let operator_id = ProfileId::try_from(req.operator_id.clone())
            .map_err(|e| Status::invalid_argument(format!("Invalid operator_id: {}", e)))?;

        let ctx = self.app_ctx.command(operator_id);

        let command = DeleteCommentCommand::try_from_proto(req)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        self.dispatch_command::<DeleteCommentCommand, (), DeleteCommentResponse>(
            &ctx,
            command,
            DeleteCommentResponse {},
        )
        .await
    }

    async fn get_root_comments(
        &self,
        request: Request<GetRootCommentsRequest>,
    ) -> Result<Response<GetCommentsResponse>, Status> {
        let req = request.into_inner();

        let post_id =
            PostId::try_from(req.post_id).map_err(|e| Status::invalid_argument(e.to_string()))?;

        let query = PageQuery {
            limit: req.limit as usize,
            cursor: req.cursor,
        };

        let ctx = self.app_ctx.query();
        let paged_result = ctx
            .find_roots_by_post(post_id, query)
            .await
            .map_err(map_domain_err_to_status)?;

        Ok(Response::new(GetCommentsResponse {
            items: paged_result
                .items
                .into_iter()
                .map(|c| c.to_proto())
                .collect(),
            next_cursor: paged_result.next_cursor,
        }))
    }

    async fn get_replies(
        &self,
        request: Request<GetRepliesRequest>,
    ) -> Result<Response<GetCommentsResponse>, Status> {
        let req = request.into_inner();

        let parent_comment_id = CommentId::try_from(req.parent_comment_id)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        let query = PageQuery {
            limit: req.limit as usize,
            cursor: req.cursor,
        };

        let ctx = self.app_ctx.query();
        let paged_result = ctx
            .find_replies_by_parent(parent_comment_id, query)
            .await
            .map_err(map_domain_err_to_status)?;

        Ok(Response::new(GetCommentsResponse {
            items: paged_result
                .items
                .into_iter()
                .map(|c| c.to_proto())
                .collect(),
            next_cursor: paged_result.next_cursor,
        }))
    }
}
