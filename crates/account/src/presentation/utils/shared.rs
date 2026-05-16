// crates/account/src/infrastructure/api/grpc/shared.rs

use crate::application::context::{AccountAppContext, AccountContext};
use shared_kernel::command::{CommandBus, IdentifiableCommand};
use shared_kernel::core::{Error, ErrorCode};
use shared_kernel::types::{AccountId, RegionCode};
use tonic::{Request, Response, Status};

#[tonic::async_trait]
pub trait GrpcServiceUtils {
    fn app_ctx(&self) -> &AccountAppContext;
    fn bus(&self) -> &CommandBus;

    /// Récupère le contexte pour une action sur un compte existant (la région est extraite de l'ID autoportant)
    fn get_context<T>(
        &self,
        _request: &Request<T>,
        account_id: &AccountId,
    ) -> Result<AccountContext, Status> {
        Ok(self.app_ctx().create_context(account_id.clone()))
    }

    /// Construit le contexte à partir des extensions (utile pour le flux de création sans ID initial)
    fn build_creation_context(
        &self,
        extensions: &tonic::Extensions,
    ) -> Result<AccountContext, Status> {
        let region = extensions
            .get::<RegionCode>()
            .cloned()
            .ok_or_else(|| Status::unauthenticated("Missing region context in extensions"))?;
        Ok(self.app_ctx().create_creation_context(region))
    }

    /// Exécute proprement une commande sur le bus et retourne le payload fourni
    async fn dispatch_command<C, Output, R>(
        &self,
        ctx: &AccountContext,
        cmd: C,
        response_payload: R,
    ) -> Result<Response<R>, Status>
    where
        C: IdentifiableCommand + std::fmt::Debug + Send + Sync + 'static + Clone,
        Output: Send + Default + 'static,
        R: Send,
    {
        self.bus()
            .execute::<AccountContext, C, Output>(ctx.clone(), cmd)
            .await
            .map_err(|err| map_domain_err_to_status(err))?;

        Ok(Response::new(response_payload))
    }

    /// Helper privé pour factoriser l'extraction de la région depuis les extensions de la requête
    fn extract_region<T>(&self, request: &Request<T>) -> Result<RegionCode, Status> {
        request
            .extensions()
            .get::<RegionCode>()
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
        ErrorCode::Unauthorized => Status::unauthenticated(error.message),
        ErrorCode::Forbidden => Status::permission_denied(error.message),
        ErrorCode::PreconditionFailed => Status::failed_precondition(error.message),
        _ => Status::internal(error.message),
    }
}
