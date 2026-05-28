// crates/profile/src/presentation/utils/shared.rs

use crate::application::context::{ProfileAppContext, ProfileCommandContext};
use crate::context::ProfileQueryContext;
use shared_kernel::command::{CommandBus, IdentifiableCommand};
use shared_kernel::core::{Error, ErrorCode};
use shared_kernel::types::{ProfileId, Region};
use tonic::{Response, Status};

#[tonic::async_trait]
pub trait GrpcServiceUtils {
    fn app_ctx(&self) -> &ProfileAppContext;
    fn bus(&self) -> &CommandBus;

    fn build_command_context(
        &self,
        profile_id: ProfileId,
        extensions: &tonic::Extensions,
    ) -> Result<ProfileCommandContext, Status> {
        let region = self.extract_region(extensions)?;
        Ok(self.app_ctx().command(profile_id, region))
    }

    fn build_creation_context(
        &self,
        extensions: &tonic::Extensions,
    ) -> Result<ProfileCommandContext, Status> {
        let region = self.extract_region(extensions)?;
        Ok(self.app_ctx().creation_command(region))
    }

    fn build_query(&self, extensions: &tonic::Extensions) -> Result<ProfileQueryContext, Status> {
        let region = self.extract_region(extensions)?;
        Ok(self.app_ctx().query(region))
    }

    async fn dispatch_command<C, Output, R>(
        &self,
        ctx: &ProfileCommandContext,
        cmd: C,
        response_payload: R,
    ) -> Result<Response<R>, Status>
    where
        C: IdentifiableCommand + std::fmt::Debug + Send + Sync + 'static + Clone,
        Output: Send + Default + 'static,
        R: Send,
    {
        self.bus()
            .execute::<ProfileCommandContext, C, Output>(ctx.clone(), cmd)
            .await
            .map_err(|err| map_domain_err_to_status(err))?;
        Ok(Response::new(response_payload))
    }

    fn extract_region(&self, ext: &tonic::Extensions) -> Result<Region, Status> {
        ext.get::<Region>()
            .cloned()
            .ok_or_else(|| Status::invalid_argument("Missing region in request extensions"))
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
