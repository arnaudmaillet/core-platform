use crate::application::context::{AccountAppContext, AccountCommandContext, AccountQueryContext};
use shared_kernel::command::{CommandBus, IdentifiableCommand};
use shared_kernel::core::{Error, ErrorCode};
use shared_kernel::types::{AccountId, Region};
use tonic::{Response, Status};

#[tonic::async_trait]
pub trait GrpcServiceUtils {
    fn app_ctx(&self) -> &AccountAppContext;
    fn bus(&self) -> &CommandBus;

    fn extract_region(&self, extensions: &tonic::Extensions) -> Result<Region, Status> {
        extensions
            .get::<Region>()
            .cloned()
            .ok_or_else(|| Status::unauthenticated("Missing region context in extensions"))
    }

    fn build_command_context(
        &self,
        account_id: AccountId,
        extensions: &tonic::Extensions,
    ) -> Result<AccountCommandContext, Status> {
        let region = self.extract_region(extensions)?;
        Ok(self.app_ctx().command(account_id, region))
    }
    
    fn build_creation_context(
        &self,
        extensions: &tonic::Extensions,
    ) -> Result<AccountCommandContext, Status> {
        let region = self.extract_region(extensions)?;
        Ok(self.app_ctx().creation_command(region))
    }

    fn build_query_context(
        &self,
        extensions: &tonic::Extensions,
    ) -> Result<AccountQueryContext, Status> {
        let region = self.extract_region(extensions)?;
        Ok(self.app_ctx().query(region))
    }

    async fn dispatch_command<C, Output, R>(
        &self,
        ctx: &AccountCommandContext,
        cmd: C,
        response_payload: R,
    ) -> Result<Response<R>, Status>
    where
        C: IdentifiableCommand + std::fmt::Debug + Send + Sync + 'static + Clone,
        Output: Send + Default + 'static,
        R: Send,
    {
        self.bus()
            .execute::<AccountCommandContext, C, Output>(ctx.clone(), cmd)
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
