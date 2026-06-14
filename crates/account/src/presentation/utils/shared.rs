use crate::application::context::{AccountCommandCtx, AccountKernelCtx, AccountQueryCtx};
use shared_kernel::command::{CommandBus, IdentifiableCommand};
use shared_kernel::core::{Error, ErrorCode};
use shared_kernel::types::{AccountId, Region};
use tonic::{Response, Status};

#[tonic::async_trait]
pub trait GrpcServiceUtils {
    fn kernel_ctx(&self) -> &AccountKernelCtx;
    fn bus(&self) -> &CommandBus;

    fn extract_region(&self, extensions: &tonic::Extensions) -> Result<Region, Status> {
        extensions
            .get::<Region>()
            .cloned()
            .ok_or_else(|| Status::unauthenticated("Missing region context in extensions"))
    }

    fn build_command_ctx(
        &self,
        account_id: AccountId,
        extensions: &tonic::Extensions,
    ) -> Result<AccountCommandCtx, Status> {
        let region = self.extract_region(extensions)?;
        Ok(self.kernel_ctx().build_command_ctx(account_id, region))
    }

    fn build_creation_ctx(
        &self,
        extensions: &tonic::Extensions,
    ) -> Result<AccountCommandCtx, Status> {
        let region = self.extract_region(extensions)?;
        Ok(self.kernel_ctx().build_creation_command_ctx(region))
    }

    fn build_query_ctx(&self, extensions: &tonic::Extensions) -> Result<AccountQueryCtx, Status> {
        let region = self.extract_region(extensions)?;
        Ok(self.kernel_ctx().build_query_ctx(region))
    }

    async fn dispatch_command<C, Output, R>(
        &self,
        ctx: &AccountCommandCtx,
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
            .execute::<AccountCommandCtx, C, Output>(ctx.clone(), cmd)
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
