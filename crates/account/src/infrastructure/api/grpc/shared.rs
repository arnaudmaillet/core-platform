// crates/account/src/infrastructure/api/grpc/shared.rs

use crate::application::context::{AccountAppContext, AccountContext};
use crate::domain::account::entities::Account;
use shared_kernel::application::{CommandBus, CommandHandler};
use shared_kernel::domain::value_objects::{AccountId, RegionCode};
use shared_kernel::errors::DomainError;
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
    ) -> Result<AccountContext, Status>
    where
        T: Send + Sync + 'static,
    {
        let region_str = request
            .metadata()
            .get("x-region")
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| Status::unauthenticated("Missing x-region header"))?;

        let region = RegionCode::try_new(region_str)
            .map_err(|_| Status::invalid_argument("Invalid region format"))?;

        Ok(self.app_ctx().create_context(account_id.clone(), region))
    }

    /// Exécute une commande et recharge l'agrégat pour renvoyer la réponse
    async fn execute_and_fetch<C, Output, R, F>(
        &self,
        ctx: &AccountContext,
        cmd: C,
        _handler: (), // On peut supprimer cet argument ou le mettre à () pour ne pas casser les appels
        mapper: F,
    ) -> Result<Response<R>, Status>
    where
        C: Send + Sync + 'static + Clone,
        Output: Send + 'static,
        R: Send,
        F: FnOnce(Account) -> R + Send + 'static,
    {
        // 1. Execute via le Bus Dynamique
        // Note le Turbofish : on passe le contexte, la commande et l'output attendu
        self.bus()
            .execute::<AccountContext, C, Output>(ctx.clone(), cmd)
            .await
            .map_err(map_domain_err_to_status)?;

        // 2. Fetch (Rechargement de l'agrégat frais depuis la DB ou le Cache)
        let account = self
            .app_ctx()
            .account_repo()
            .find_by_id(ctx.account_id(), None)
            .await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?
            .ok_or_else(|| Status::not_found("Account not found after operation"))?;

        // 3. Map vers le message Protobuf
        Ok(Response::new(mapper(account)))
    }
}

pub fn map_domain_err_to_status(err: DomainError) -> Status {
    match err {
        DomainError::NotFound { .. } => Status::not_found(err.to_string()),
        DomainError::Forbidden { .. } => Status::permission_denied(err.to_string()),
        DomainError::ConcurrencyConflict { .. } => Status::aborted("Conflict, please retry"),
        _ => Status::internal("Internal error"),
    }
}
