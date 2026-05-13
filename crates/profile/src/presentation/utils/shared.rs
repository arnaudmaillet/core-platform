// crates/profile/src/presentation/utils/shared.rs

use crate::application::context::{ProfileAppContext, ProfileContext};
use crate::entities::Profile;
use crate::value_objects::ProfileId;
use shared_kernel::application::{CommandBus, IdentifiableCommand};
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
        // On récupère l'objet RegionCode que l'intercepteur a mis dedans
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
        Output: Send + 'static,
        R: Send,
        F: FnOnce(Profile) -> R + Send + 'static,
    {
        let execution_result = self
            .bus()
            .execute::<ProfileContext, C, Output>(ctx.clone(), cmd)
            .await;

        if let Err(err) = execution_result {
            let error: Error = err.clone().into();

            // Gestion de l'idempotence technique (Command déjà traitée)
            if error.code == ErrorCode::AlreadyExists && error.message.contains("Command") {
                tracing::info!(
                    profile_id = %ctx.region(),
                    "🔁 Idempotency hit. Fetching current state."
                );
            } else {
                return Err(map_domain_err_to_status(err));
            }
        }

        let profile = self
            .app_ctx()
            .profile_repo()
            .find_by_id(ctx.profile_id(), ctx.region(), None)
            .await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?
            .ok_or_else(|| Status::not_found("Profile not found"))?;

        // 3. Map vers le proto de réponse
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
