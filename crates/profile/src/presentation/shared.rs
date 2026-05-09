// crates/profile/src/presentation/shared.rs

use crate::application::context::{ProfileAppContext, ProfileContext};
use crate::domain::entities::Profile;
use crate::domain::value_objects::ProfileId;
use shared_kernel::application::{CommandBus, IdentifiableCommand};
use shared_kernel::errors::{AppError, DomainError, ErrorCode};
use tonic::{Request, Response, Status};

#[tonic::async_trait]
pub trait GrpcServiceUtils {
    fn app_ctx(&self) -> &ProfileAppContext;
    fn bus(&self) -> &CommandBus;

    async fn get_context<T>(
        &self,
        request: &Request<T>,
        profile_id: &ProfileId,
    ) -> Result<ProfileContext, Status>
    where
        T: Send + Sync + 'static,
    {
        // On pourrait aussi extraire l'user_id du token ici si besoin
        Ok(self.app_ctx().create_context(profile_id.clone()))
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
        // 1. Execute via le Bus (qui inclut maintenant le logging et le retry)
        let execution_result = self
            .bus()
            .execute::<ProfileContext, C, Output>(ctx.clone(), cmd)
            .await;

        if let Err(err) = execution_result {
            let app_error: AppError = err.clone().into();

            // Gestion de l'idempotence technique (Command déjà traitée)
            if app_error.code == ErrorCode::AlreadyExists && app_error.message.contains("Command") {
                tracing::info!(
                    profile_id = %ctx.profile_id(),
                    "🔁 Idempotency hit. Fetching current state."
                );
            } else {
                return Err(map_domain_err_to_status(err));
            }
        }

        // 2. Fetch l'agrégat Profile
        let profile = self
            .app_ctx()
            .profile_repo()
            .find_by_id(ctx.profile_id(), None)
            .await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?
            .ok_or_else(|| Status::not_found("Profile not found"))?;

        // 3. Map vers le proto de réponse
        Ok(Response::new(mapper(profile)))
    }
}

// Cette fonction devrait idéalement être dans shared_kernel::infrastructure::grpc
pub fn map_domain_err_to_status(err: DomainError) -> Status {
    let app_error: AppError = err.into();
    match app_error.code {
        ErrorCode::NotFound => Status::not_found(app_error.message),
        ErrorCode::AlreadyExists => Status::already_exists(app_error.message),
        ErrorCode::ValidationFailed => Status::invalid_argument(app_error.message),
        ErrorCode::ConcurrencyConflict => Status::aborted(app_error.message),
        _ => Status::internal(app_error.message),
    }
}
