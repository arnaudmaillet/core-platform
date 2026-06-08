// crates/content_comments/src/presentation/utils.rs

use shared_kernel::command::{CacheKeyComponent, CommandBus, IdentifiableCommand};
use shared_kernel::core::{Error, ErrorCode};
use tonic::{Response, Status};

use crate::application::context::{CommentAppContext, CommentCommandContext};

pub trait GrpcServiceUtils {
    type AppContext;

    fn app_ctx(&self) -> &CommentAppContext;
    fn bus(&self) -> &CommandBus;

    async fn dispatch_command<C, Output, R>(
        &self,
        ctx: &CommentCommandContext,
        cmd: C,
        response_payload: R,
    ) -> Result<Response<R>, Status>
    where
        C: IdentifiableCommand + std::fmt::Debug + Send + Sync + 'static + Clone,
        C::Routing: CacheKeyComponent,
        C::Id: std::fmt::Display,
        Output: Send + Default + 'static,
        R: Send,
    {
        self.bus()
            .execute::<CommentCommandContext, C, Output>(ctx.clone(), cmd)
            .await
            .map_err(map_domain_err_to_status)?;

        Ok(Response::new(response_payload))
    }
}

pub fn map_domain_err_to_status(err: Error) -> Status {
    match err.code {
        ErrorCode::NotFound => Status::not_found(err.message),
        ErrorCode::AlreadyExists => Status::already_exists(err.message),
        ErrorCode::ValidationFailed => Status::invalid_argument(err.message),
        ErrorCode::ConcurrencyConflict => Status::aborted(err.message),
        ErrorCode::Unauthorized => Status::unauthenticated(err.message),
        ErrorCode::Forbidden => Status::permission_denied(err.message),
        ErrorCode::PreconditionFailed => Status::failed_precondition(err.message),
        _ => Status::internal(err.message),
    }
}
