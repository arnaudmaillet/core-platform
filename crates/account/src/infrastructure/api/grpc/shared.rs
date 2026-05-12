// crates/account/src/infrastructure/api/grpc/shared.rs

use crate::application::context::{AccountAppContext, AccountContext};
use crate::domain::account::entities::Account;
use shared_kernel::application::{CommandBus, CommandHandler};
use shared_kernel::domain::value_objects::{AccountId, RegionCode};
use shared_kernel::core::{AppError, DomainError, ErrorCode};
use tonic::{Request, Response, Status};

// crates/account/src/infrastructure/api/grpc/shared.rs

#[tonic::async_trait]
pub trait GrpcServiceUtils {
    fn app_ctx(&self) -> &AccountAppContext;
    fn bus(&self) -> &CommandBus;

    async fn get_context<T>(
        &self,
        request: &Request<T>,
        account_id: &AccountId,
    ) -> Result<AccountContext, Status> {
        let region = request.extensions().get::<RegionCode>().ok_or_else(|| {
            Status::unauthenticated("Missing region context (not injected by interceptor)")
        })?;

        Ok(self.app_ctx().create_context(account_id.clone()))
    }

    /// Exécute une commande et recharge l'agrégat pour renvoyer la réponse
    async fn execute_and_fetch<C, Output, R, F>(
        &self,
        ctx: &AccountContext,
        cmd: C,
        _handler: (),
        mapper: F,
    ) -> Result<Response<R>, Status>
    where
        C: Send + Sync + 'static + Clone,
        Output: Send + 'static,
        R: Send,
        F: FnOnce(Account) -> R + Send + 'static,
    {
        // 1. Execute
        let execution_result = self
            .bus()
            .execute::<AccountContext, C, Output>(ctx.clone(), cmd)
            .await;

        if let Err(err) = execution_result {
            let app_error: AppError = err.clone().into();

            // --- LOGIQUE D'IDEMPOTENCE GÉNEEIQUE ---
            // Si la commande a déjà été traitée, on ne renvoie pas d'erreur.
            // On considère que c'est un "succès" et on passe au Fetch.
            if app_error.code == ErrorCode::AlreadyExists && app_error.message.contains("Command") {
                tracing::info!(
                    "🔁 Idempotency hit for account {:?}. Skipping execution, fetching current state.",
                    ctx.account_id()
                );
            } else {
                // Pour toutes les autres erreurs, on conserve le comportement habituel
                return Err(map_domain_err_to_status(err));
            }
        }

        // 2. Fetch (Inchangé, mais appelé même en cas d'idempotency hit)
        let search_id = ctx.account_id();
        let account = self
            .app_ctx()
            .account_repo()
            .find_by_id(search_id, None)
            .await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?
            .ok_or_else(|| Status::not_found(format!("Account not found: {:?}", search_id)))?;

        // 3. Map
        Ok(Response::new(mapper(account)))
    }
}

pub fn map_domain_err_to_status(err: DomainError) -> Status {
    let app_error: AppError = err.into();

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
