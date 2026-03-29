// crates/account/src/infrastructure/api/grpc/mappers/errors_mapper.rs

use tonic::Status;
use shared_kernel::errors::{DomainError, Result as DomainResult};

pub trait ToGrpcStatus<T> {
    fn map_grpc(self) -> Result<T, Status>;
}

impl<T> ToGrpcStatus<T> for DomainResult<T> {
    fn map_grpc(self) -> Result<T, Status> {
        self.map_err(|e| {
            // Log de l'erreur interne pour le debugging (optionnel)
            // tracing::error!(error = ?e, "Domain error mapped to gRPC status");

            match e {
                DomainError::NotFound { entity, id } => 
                    Status::not_found(format!("{entity} '{id}' not found")),

                DomainError::AlreadyExists { entity, field, value } => 
                    Status::already_exists(format!("{entity} with {field} '{value}' already exists")),

                DomainError::Validation { field, reason } => 
                    Status::invalid_argument(format!("Invalid {field}: {reason}")),

                DomainError::Forbidden { reason } => 
                    Status::permission_denied(reason),

                DomainError::Unauthorized { reason } => 
                    Status::unauthenticated(reason),

                DomainError::ConcurrencyConflict { reason } => 
                    Status::aborted(format!("Conflict: {reason}")),

                // Pour les cas comme "Compte banni" ou "Email non vérifié"
                DomainError::PreconditionFailed { reason } => 
                    Status::failed_precondition(reason),

                // Tout le reste (DB, Kafka, Network) devient une 500 interne
                _ => Status::internal("An internal server error occurred"),
            }
        })
    }
}