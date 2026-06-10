// crates/profile/src/presentation/utils/shared.rs

use crate::context::{ProfileAppContext, ProfileCommandContext, ProfileQueryContext};
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
        let routed_region = self.extract_region(extensions)?;
        let local_region = self.app_ctx().local_region();
        if routed_region != local_region {
            return Err(Status::failed_precondition(format!(
                "Routing violation: This pod ({:?}) cannot process data belonging to region {:?}",
                local_region, routed_region
            )));
        }

        Ok(self.app_ctx().command(profile_id))
    }

    fn build_creation_context(
        &self,
        extensions: &tonic::Extensions,
    ) -> Result<ProfileCommandContext, Status> {
        let routed_region = self.extract_region(extensions)?;
        let local_region = self.app_ctx().local_region();

        if routed_region != local_region {
            return Err(Status::failed_precondition(format!(
                "Routing violation: Creation request for region {:?} landed on pod region {:?}",
                routed_region, local_region
            )));
        }

        Ok(self.app_ctx().creation_command())
    }

    fn build_query(&self, _extensions: &tonic::Extensions) -> Result<ProfileQueryContext, Status> {
        Ok(self.app_ctx().query())
    }

    async fn dispatch_command<C, Output, R>(
        &self,
        ctx: &ProfileCommandContext,
        cmd: C,
        response_payload: R,
    ) -> Result<Response<R>, Status>
    where
        C: IdentifiableCommand + std::fmt::Debug + Send + Sync + 'static + Clone,
        C::Routing: shared_kernel::command::CacheKeyComponent,
        Output: Send + Sync + Default + Clone + 'static,
        R: Send,
    {
        self.bus()
            .execute::<ProfileCommandContext, C, Output>(ctx.clone(), cmd)
            .await
            .map_err(map_domain_err_to_status)?;

        Ok(Response::new(response_payload))
    }

    fn extract_region(&self, ext: &tonic::Extensions) -> Result<Region, Status> {
        ext.get::<Region>().cloned().ok_or_else(|| {
            Status::invalid_argument(
                "Infrastructure error: Missing routed region in request extensions",
            )
        })
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
