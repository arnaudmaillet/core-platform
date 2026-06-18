use shared_kernel::core::{Error, ErrorCode};
use shared_kernel::types::Region;
use tonic::Status;

pub trait GrpcQueryUtils {
    fn extract_region(&self, extensions: &tonic::Extensions) -> Result<Region, Status> {
        extensions
            .get::<Region>()
            .cloned()
            .ok_or_else(|| Status::unauthenticated("Missing region context in extensions"))
    }
}

pub fn map_domain_err_to_status(err: Error) -> Status {
    match err.code {
        ErrorCode::NotFound => Status::not_found(err.message),
        ErrorCode::Unauthorized => Status::unauthenticated(err.message),
        ErrorCode::Forbidden => Status::permission_denied(err.message),
        ErrorCode::ValidationFailed => Status::invalid_argument(err.message),
        // Le serveur de query traite moins de cas d'erreur de transition (comme ConcurrencyConflict),
        // mais conserver le même matching garantit une traduction cohérente des erreurs du domaine.
        _ => Status::internal(err.message),
    }
}
