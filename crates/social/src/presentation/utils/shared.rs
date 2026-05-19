use crate::context::{SocialAppContext, SocialContext};
use shared_kernel::command::{CommandBus, IdentifiableCommand};
use shared_kernel::core::{Error, ErrorCode};
use shared_kernel::types::{ProfileId, Region};
use tonic::{Request, Response, Status};

#[tonic::async_trait]
pub trait GrpcServiceUtils {
    type AppContext;

    fn app_ctx(&self) -> &SocialAppContext;
    fn bus(&self) -> &CommandBus;

    fn get_context<T>(
        &self,
        request: &Request<T>,
        profile_id: ProfileId,
    ) -> Result<SocialContext, Status> {
        let region = self.extract_region(request)?;
        Ok(self.app_ctx().create_context(profile_id, region))
    }

    fn build_context(
        &self,
        profile_id: ProfileId,
        extensions: &tonic::Extensions,
    ) -> Result<SocialContext, Status> {
        let region = extensions
            .get::<Region>()
            .cloned()
            .ok_or_else(|| Status::unauthenticated("Missing region context in extensions"))?;
        Ok(self.app_ctx().create_context(profile_id, region))
    }

    async fn dispatch_command<C, Output, R>(
        &self,
        ctx: &SocialContext,
        cmd: C,
        response_payload: R,
    ) -> Result<Response<R>, Status>
    where
        C: IdentifiableCommand + std::fmt::Debug + Send + Sync + 'static + Clone,
        Output: Send + Default + 'static,
        R: Send,
    {
        self.bus()
            .execute::<SocialContext, C, Output>(ctx.clone(), cmd)
            .await
            .map_err(|err| map_domain_err_to_status(err))?;
        Ok(Response::new(response_payload))
    }

    /// Helper privé pour factoriser l'extraction de la région depuis les extensions de la requête
    fn extract_region<T>(&self, request: &Request<T>) -> Result<Region, Status> {
        request
            .extensions()
            .get::<Region>()
            .cloned()
            .ok_or_else(|| Status::unauthenticated("Missing region context in request extensions"))
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
