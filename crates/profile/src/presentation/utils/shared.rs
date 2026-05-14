// crates/profile/src/presentation/utils/shared.rs

use crate::application::context::{ProfileAppContext, ProfileContext};
use crate::entities::Profile;
use crate::types::ProfileId;
use shared_kernel::command::{CommandBus, IdentifiableCommand};
use shared_kernel::core::{Error, ErrorCode};
use shared_kernel::types::RegionCode;
use tonic::{Request, Response, Status};

#[tonic::async_trait]
pub trait GrpcServiceUtils {
    fn app_ctx(&self) -> &ProfileAppContext;
    fn bus(&self) -> &CommandBus;

    fn get_context<T>(
        &self,
        request: &Request<T>,
        profile_id: &ProfileId,
    ) -> Result<ProfileContext, Status> {
        let region = request
            .extensions()
            .get::<RegionCode>()
            .cloned()
            .ok_or_else(|| Status::unauthenticated("Missing region context"))?;

        Ok(self.app_ctx().create_context(profile_id.clone(), region))
    }

    async fn execute_and_fetch<C, Output, R, F>(
        &self,
        ctx: &ProfileContext,
        cmd: C,
        mapper: F,
    ) -> Result<Response<R>, Status>
    where
        C: IdentifiableCommand + std::fmt::Debug + Send + Sync + 'static + Clone,
        Output: Send + Default + 'static, // On ajoute Default pour le Bus
        R: Send,
        F: FnOnce(Profile) -> R + Send + 'static,
    {
        // 1. Exécution (Le Bus gère le AlreadyExists et renvoie Ok)
        self.bus()
            .execute::<ProfileContext, C, Output>(ctx.clone(), cmd)
            .await
            .map_err(|err| map_domain_err_to_status(err))?;

        // 2. Fetch de l'état actuel
        let profile = self
            .app_ctx()
            .profile_repo()
            .find_by_id(ctx.profile_id(), ctx.region(), None)
            .await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?
            .ok_or_else(|| Status::not_found("Profile not found"))?;

        // 3. Mapping
        Ok(Response::new(mapper(profile)))
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
