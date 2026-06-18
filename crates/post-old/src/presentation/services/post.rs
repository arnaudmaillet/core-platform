// crates/post/src/presentation/grpc/post_service.rs

use shared_kernel::core::PageQuery;
use shared_kernel::types::{PostId, ProfileId};
use std::sync::Arc;
use tonic::{Request, Response, Status};

use crate::commands::{
    ChangeVisibilityCommand, CreatePostCommand, DeletePostCommand, ToggleCommentsCommand,
    UpdateCaptionCommand,
};
use crate::context::PostKernelCtx;
use crate::presentation::mappers::GrpcPostMapper;
use crate::utils::{GrpcServiceUtils, map_domain_err_to_status};
use shared_kernel::command::CommandBus;
use shared_proto::post::v1::post_service_server::PostService as ProtoPostService;
use shared_proto::post::v1::*;

pub struct PostService {
    bus: CommandBus,
    app_ctx: PostKernelCtx,
}

impl PostService {
    pub fn new(bus: CommandBus, app_ctx: PostKernelCtx) -> Self {
        Self { bus, app_ctx }
    }
}

impl GrpcServiceUtils for PostService {
    fn kernel_ctx(&self) -> &PostKernelCtx {
        &self.app_ctx
    }
    fn bus(&self) -> &CommandBus {
        &self.bus
    }
}

#[tonic::async_trait]
impl ProtoPostService for PostService {
    async fn create_post(
        &self,
        request: Request<CreatePostRequest>,
    ) -> Result<Response<CreatePostResponse>, Status> {
        let (_meta, ext, req) = request.into_parts();
        let author_id = ProfileId::try_new(req.author_id.clone())
            .map_err(|e| Status::invalid_argument(e.to_string()))?;
        let post_id = PostId::generate();
        let ctx = self.build_command_ctx(author_id, &ext)?;

        let command = CreatePostCommand::try_from_proto(req, post_id, ctx.region())
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        self.dispatch_command::<CreatePostCommand, (), CreatePostResponse>(
            &ctx,
            command,
            CreatePostResponse {
                post_id: post_id.to_string(),
            },
        )
        .await
    }

    async fn update_caption(
        &self,
        request: Request<UpdateCaptionRequest>,
    ) -> Result<Response<UpdateCaptionResponse>, Status> {
        let (_meta, ext, req) = request.into_parts();
        let target = req
            .target
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Missing target"))?;

        let author_id = target
            .author_id
            .parse::<ProfileId>()
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        let ctx = self.build_command_ctx(author_id, &ext)?;

        let command = UpdateCaptionCommand::try_from_proto(req, ctx.region())
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        self.dispatch_command::<UpdateCaptionCommand, (), UpdateCaptionResponse>(
            &ctx,
            command,
            UpdateCaptionResponse {},
        )
        .await
    }

    async fn change_visibility(
        &self,
        request: Request<ChangeVisibilityRequest>,
    ) -> Result<Response<ChangeVisibilityResponse>, Status> {
        let (_meta, ext, req) = request.into_parts();
        let target = req
            .target
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Missing target"))?;

        let author_id = target
            .author_id
            .parse::<ProfileId>()
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        let ctx = self.build_command_ctx(author_id, &ext)?;

        let command = ChangeVisibilityCommand::try_from_proto(req, ctx.region())
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        self.dispatch_command::<ChangeVisibilityCommand, (), ChangeVisibilityResponse>(
            &ctx,
            command,
            ChangeVisibilityResponse {},
        )
        .await
    }

    async fn toggle_comments(
        &self,
        request: Request<ToggleCommentsRequest>,
    ) -> Result<Response<ToggleCommentsResponse>, Status> {
        let (_meta, ext, req) = request.into_parts();
        let target = req
            .target
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Missing target"))?;

        let author_id = target
            .author_id
            .parse::<ProfileId>()
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        let ctx = self.build_command_ctx(author_id, &ext)?;

        let command = ToggleCommentsCommand::try_from_proto(req, ctx.region())
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        self.dispatch_command::<ToggleCommentsCommand, (), ToggleCommentsResponse>(
            &ctx,
            command,
            ToggleCommentsResponse {},
        )
        .await
    }

    async fn delete_post(
        &self,
        request: Request<DeletePostRequest>,
    ) -> Result<Response<DeletePostResponse>, Status> {
        let (_meta, ext, req) = request.into_parts();
        let target = req
            .target
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Missing target"))?;

        let author_id = target
            .author_id
            .parse::<ProfileId>()
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        let ctx = self.build_command_ctx(author_id, &ext)?;

        let command = DeletePostCommand::try_from_proto(req, ctx.region())
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        self.dispatch_command::<DeletePostCommand, (), DeletePostResponse>(
            &ctx,
            command,
            DeletePostResponse {},
        )
        .await
    }

    async fn get_post(
        &self,
        request: Request<GetPostRequest>,
    ) -> Result<Response<shared_proto::post::v1::Post>, Status> {
        let (_meta, ext, req) = request.into_parts();
        let post_id =
            PostId::try_from(req.post_id).map_err(|e| Status::invalid_argument(e.to_string()))?;

        let ctx = self.build_query_ctx(&ext)?;
        let post = ctx
            .find_by_id(&post_id)
            .await
            .map_err(map_domain_err_to_status)?
            .ok_or_else(|| Status::not_found("Post not found"))?;

        let proto_post = GrpcPostMapper::to_proto(&post);
        Ok(Response::new(proto_post))
    }

    async fn get_posts_by_author(
        &self,
        request: Request<GetPostsByAuthorRequest>,
    ) -> Result<Response<GetPostsByAuthorResponse>, Status> {
        let (_meta, ext, req) = request.into_parts();

        let author_id = ProfileId::try_new(req.author_id.clone())
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        let ctx = self.build_query_ctx(&ext)?;

        let query = PageQuery {
            limit: req.limit as usize,
            cursor: req.cursor,
        };

        let paged_posts = ctx
            .find_by_author(&author_id, query)
            .await
            .map_err(map_domain_err_to_status)?;

        Ok(Response::new(GetPostsByAuthorResponse {
            items: paged_posts
                .items
                .iter()
                .map(GrpcPostMapper::to_proto)
                .collect(),
            next_cursor: paged_posts.next_cursor,
        }))
    }
}
