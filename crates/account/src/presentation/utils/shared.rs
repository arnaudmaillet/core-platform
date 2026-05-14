// crates/account/src/infrastructure/api/grpc/shared.rs

use crate::application::context::{AccountAppContext, AccountContext};
use crate::domain::entities::Account;
use shared_kernel::command::{CommandBus, IdentifiableCommand};
use shared_kernel::core::{Error, ErrorCode};
use shared_kernel::types::{AccountId, RegionCode};
use tonic::{Request, Response, Status};

#[tonic::async_trait]
pub trait GrpcServiceUtils {
    fn app_ctx(&self) -> &AccountAppContext;
    fn bus(&self) -> &CommandBus;

    fn get_context<T>(
        &self,
        request: &Request<T>,
        account_id: &AccountId,
    ) -> Result<AccountContext, Status> {
        let _region = request.extensions().get::<RegionCode>().ok_or_else(|| {
            Status::unauthenticated("Missing region context (not injected by interceptor)")
        })?;

        // AccountContext utilise l'ID pour le scoping
        Ok(self.app_ctx().create_context(account_id.clone()))
    }

    /// Exécute une commande et recharge l'agrégat pour renvoyer la réponse
    async fn execute_and_fetch<C, Output, R, F>(
        &self,
        ctx: &AccountContext,
        cmd: C,
        mapper: F,
    ) -> Result<Response<R>, Status>
    where
        C: IdentifiableCommand + std::fmt::Debug + Send + Sync + 'static + Clone,
        Output: Send + Default + 'static,
        R: Send,
        F: FnOnce(Account) -> R + Send + 'static,
    {
        // 1. Exécution
        self.bus()
            .execute::<AccountContext, C, Output>(ctx.clone(), cmd)
            .await
            .map_err(|err| map_domain_err_to_status(err))?;

        // 2. Fetch
        let account = self
            .app_ctx()
            .account_repo()
            .find_by_id(ctx.account_id(), None)
            .await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?
            .ok_or_else(|| Status::not_found("Account not found"))?;

        // 3. Mapping
        Ok(Response::new(mapper(account)))
    }
}

pub fn map_domain_err_to_status(err: Error) -> Status {
    let app_error: Error = err.into();

    match app_error.code {
        ErrorCode::NotFound => Status::not_found(app_error.message),
        ErrorCode::AlreadyExists => Status::already_exists(app_error.message),
        ErrorCode::Unauthorized => Status::unauthenticated(app_error.message),
        ErrorCode::Forbidden => Status::permission_denied(app_error.message),
        ErrorCode::ValidationFailed => Status::invalid_argument(app_error.message),
        ErrorCode::ConcurrencyConflict => Status::aborted(app_error.message),
        ErrorCode::PreconditionFailed => Status::failed_precondition(app_error.message),
        _ => Status::internal(format!("Internal Server Error: {}", app_error.message)),
    }
}
