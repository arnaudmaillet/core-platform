// crates/profile/src/infrastructure/api/grpc/mappers/error_mapper.rs

use tonic::Status;
use shared_kernel::errors::DomainError;

pub trait ToGrpcStatus<T> {
    fn map_grpc(self) -> Result<T, Status>;
}

impl<T> ToGrpcStatus<T> for shared_kernel::errors::Result<T> {
    fn map_grpc(self) -> Result<T, Status> {
        self.map_err(|e| match e {
            DomainError::NotFound { entity, id } =>
                Status::not_found(format!("{} '{}' not found", entity, id)),

            DomainError::AlreadyExists { entity, field, value } =>
                Status::already_exists(format!("{} with {} '{}' already exists", entity, field, value)),

            DomainError::Validation { field, reason } =>
                Status::invalid_argument(format!("Validation failed for {}: {}", field, reason)),

            DomainError::ConcurrencyConflict { reason } =>
                Status::aborted(format!("Concurrency conflict: {}", reason)),

            DomainError::Forbidden { reason } =>
                Status::permission_denied(reason),

            _ => Status::internal(format!("Infrastructure failure: {}", e)),
        })
    }
}