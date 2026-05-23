// crates/profile/src/presentation/utils/shared.rs

use crate::application::context::{ProfileAppContext, ProfileContext};
use shared_kernel::command::{CommandBus, IdentifiableCommand};
use shared_kernel::core::{Error, ErrorCode};
use shared_kernel::types::{ProfileId, Region};
use tonic::{Response, Status};

#[tonic::async_trait]
pub trait GrpcServiceUtils {
    fn app_ctx(&self) -> &ProfileAppContext;
    fn bus(&self) -> &CommandBus;

    fn build_context(
        &self,
        profile_id: ProfileId,
        extensions: &tonic::Extensions,
    ) -> Result<ProfileContext, Status> {
        let region = extensions
            .get::<Region>()
            .cloned()
            .ok_or_else(|| Status::unauthenticated("Missing region context in extensions"))?;
        Ok(self.app_ctx().create_context(profile_id, region))
    }

    async fn dispatch_command<C, Output, R>(
        &self,
        ctx: &ProfileContext,
        cmd: C,
        response_payload: R,
    ) -> Result<Response<R>, Status>
    where
        C: IdentifiableCommand + std::fmt::Debug + Send + Sync + 'static + Clone,
        Output: Send + Default + 'static,
        R: Send,
    {
        self.bus()
            .execute::<ProfileContext, C, Output>(ctx.clone(), cmd)
            .await
            .map_err(|err| map_domain_err_to_status(err))?;
        Ok(Response::new(response_payload))
    }
}

pub fn map_domain_err_to_status(err: Error) -> Status {
    let error: Error = err.into();
    match error.code {
        ErrorCode::NotFound => Status::not_found(error.message),
        ErrorCode::AlreadyExists => Status::already_exists(error.message),
        ErrorCode::ValidationFailed => Status::invalid_argument(error.message),
        ErrorCode::ConcurrencyConflict => Status::aborted(error.message),
        _ => Status::internal(error.message),
    }
}
