// crates/identity/src/errors.rs

use thiserror::Error;

#[derive(Error, Debug, Clone, PartialEq)]
pub enum DomainError {
    #[error("validation failed for field '{field}': {reason}")]
    Validation {
        field: &'static str,
        reason: String,
    },

    #[error("{entity} not found with id '{id}'")]
    NotFound {
        entity: &'static str,
        id: String,
    },

    #[error("{entity} already exists with {field} = '{value}'")]
    AlreadyExists {
        entity: &'static str,
        field: &'static str,
        value: String,
    },

    #[error("unauthorized: {reason}")]
    Unauthorized { reason: String },

    #[error("internal domain error: {0}")]
    Internal(String),
}

pub type Result<T> = std::result::Result<T, DomainError>;