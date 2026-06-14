// crates/profile/src/presentation/utils/shared.rs

use crate::context::{ProfileCommandCtx, ProfileKernelCtx, ProfileQueryCtx};
use shared_kernel::command::{CommandBus, IdentifiableCommand};
use shared_kernel::core::{Error, ErrorCode};
use shared_kernel::types::{ProfileId, Region};
use tonic::{Response, Status};

#[tonic::async_trait]
pub trait GrpcServiceUtils {
    fn kernel(&self) -> &ProfileKernelCtx;
    fn bus(&self) -> &CommandBus;

    fn build_command_ctx(
        &self,
        profile_id: ProfileId,
        extensions: &tonic::Extensions,
    ) -> Result<ProfileCommandCtx, Status> {
        let command_region = self.extract_region(extensions)?;
        let server_region = self.kernel().server_region();

        // Barrière gRPC Fail-Fast pour le Sharding
        if command_region != server_region {
            return Err(Status::failed_precondition(format!(
                "Routing violation: This pod ({:?}) cannot process data belonging to region {:?}",
                server_region, command_region
            )));
        }

        Ok(ProfileCommandCtx::new(
            self.kernel().clone(),
            Some(profile_id),
            command_region,
        ))
    }

    fn build_creation_ctx(
        &self,
        extensions: &tonic::Extensions,
    ) -> Result<ProfileCommandCtx, Status> {
        let query_region = self.extract_region(extensions)?;
        let server_region = self.kernel().server_region();

        if query_region != server_region {
            return Err(Status::failed_precondition(format!(
                "Routing violation: Creation request for region {:?} landed on pod region {:?}",
                query_region, server_region
            )));
        }

        Ok(self.kernel().creation_command(query_region))
    }

    fn build_query_ctx(&self, _extensions: &tonic::Extensions) -> Result<ProfileQueryCtx, Status> {
        Ok(ProfileQueryCtx::new(self.kernel().clone()))
    }

    async fn dispatch_command<C, Output, R>(
        &self,
        ctx: &ProfileCommandCtx,
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
            .execute::<ProfileCommandCtx, C, Output>(ctx.clone(), cmd)
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
