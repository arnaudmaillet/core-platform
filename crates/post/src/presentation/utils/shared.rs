// crates/post/src/application/utils.rs

use crate::application::context::{PostAppContext, PostCommandContext};
use shared_kernel::command::{CommandBus, IdentifiableCommand};
use shared_kernel::core::{Error, ErrorCode};
use shared_kernel::types::{PostId, ProfileId, Region};
use tonic::{Response, Status};

pub trait GrpcServiceUtils {
    fn app_ctx(&self) -> &PostAppContext;
    fn bus(&self) -> &CommandBus;

    fn build_context(
        &self,
        author_id: ProfileId,
        post_id: PostId,
        extensions: &tonic::Extensions,
    ) -> Result<PostCommandContext, Status> {
        let region = self.extract_region(extensions)?;
        Ok(self.app_ctx().command(author_id, post_id, region))
    }

    async fn dispatch_command<C, Output, R>(
        &self,
        ctx: &PostCommandContext,
        cmd: C,
        response_payload: R,
    ) -> Result<Response<R>, Status>
    where
        C: IdentifiableCommand + std::fmt::Debug + Send + Sync + 'static + Clone,
        Output: Send + Default + 'static,
        R: Send,
    {
        self.bus()
            .execute::<PostCommandContext, C, Output>(ctx.clone(), cmd)
            .await
            .map_err(map_domain_err_to_status)?;

        Ok(Response::new(response_payload))
    }

    fn extract_region(&self, ext: &tonic::Extensions) -> Result<Region, Status> {
        ext.get::<Region>()
            .cloned()
            .ok_or_else(|| Status::invalid_argument("Missing region in request extensions"))
    }
}

pub fn map_domain_err_to_status(err: Error) -> Status {
    match err.code {
        ErrorCode::NotFound => Status::not_found(err.message),
        ErrorCode::AlreadyExists => Status::already_exists(err.message),
        ErrorCode::ValidationFailed => Status::invalid_argument(err.message),
        ErrorCode::ConcurrencyConflict => Status::aborted(err.message),
        _ => Status::internal(err.message),
    }
}
