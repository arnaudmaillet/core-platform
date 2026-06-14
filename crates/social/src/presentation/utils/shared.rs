// crates/social/src/presentation/utils/shared.rs

use crate::application::context::{SocialCommandCtx, SocialKernelCtx, SocialQueryCtx};
use crate::domain::repositories::ProfileCountersStorageRepository; // Import du trait de stockage
use shared_kernel::command::{CommandBus, IdentifiableCommand};
use shared_kernel::core::{Error, ErrorCode};
use shared_kernel::types::{ProfileId, Region};
use std::sync::Arc;
use tonic::{Response, Status};

#[tonic::async_trait]
pub trait GrpcServiceUtils {
    type AppContext;

    fn kernel(&self) -> &SocialKernelCtx;
    fn bus(&self) -> &CommandBus;
    fn profile_counters_storage(&self) -> &Arc<dyn ProfileCountersStorageRepository>;

    fn extract_region(&self, extensions: &tonic::Extensions) -> Result<Region, Status> {
        extensions
            .get::<Region>()
            .cloned()
            .ok_or_else(|| Status::unauthenticated("Missing region context in extensions"))
    }

    fn build_command_ctx(
        &self,
        profile_id: ProfileId,
        extensions: &tonic::Extensions,
    ) -> Result<SocialCommandCtx, Status> {
        let region = self.extract_region(extensions)?;
        Ok(SocialCommandCtx::new(
            self.kernel().clone(),
            profile_id,
            region,
        ))
    }

    fn build_query_ctx(&self, extensions: &tonic::Extensions) -> Result<SocialQueryCtx, Status> {
        let region = self.extract_region(extensions)?;

        Ok(SocialQueryCtx::new(
            self.kernel().clone(),
            self.profile_counters_storage().clone(),
            region,
        ))
    }

    async fn dispatch_command<C, Output, R>(
        &self,
        ctx: &SocialCommandCtx,
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
            .execute::<SocialCommandCtx, C, Output>(ctx.clone(), cmd)
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
