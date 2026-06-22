use crate::utils::GrpcServiceUtils;
use post_older::{
    ChangeVisibilityCommand, CreatePostCommand, DeletePostCommand, PostKernelCtx,
    ToggleCommentsCommand, UpdateCaptionCommand,
};
use post_assembly::PostCommandContainer;
use post_proto_bridge::v1::post_command_service_server::PostCommandService as ProtoPostCommandService;
use post_proto_bridge::v1::*;
use shared_kernel::command::CommandBus;
use shared_kernel::types::{PostId, ProfileId};
use tonic::{Request, Response, Status};

pub struct PostCommandService {
    container: PostCommandContainer,
}

impl PostCommandService {
    pub fn new(container: PostCommandContainer) -> Self {
        Self { container }
    }
}

impl GrpcServiceUtils for PostCommandService {
    fn kernel_ctx(&self) -> &PostKernelCtx {
        &self.container.kernel_ctx
    }
    fn bus(&self) -> &CommandBus {
        &self.container.bus
    }
}

#[tonic::async_trait]
impl ProtoPostCommandService for PostCommandService {
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
}
