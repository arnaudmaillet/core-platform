// crates/shared-kernel/src/errors/domain_error.rs

use thiserror::Error;
use crate::errors::AppError;

#[derive(Error, Debug, Clone, PartialEq)]
pub enum DomainError {
    #[error("Validation failed for field '{field}': {reason}")]
    Validation {
        field: &'static str,
        reason: String
    },

    #[error("{entity} not found with id '{id}'")]
    NotFound {
        entity: &'static str,
        id: String
    },

    #[error("{entity} already exists with {field} = '{value}'")]
    AlreadyExists {
        entity: &'static str,
        field: &'static str,
        value: String
    },

    /// Erreur de concurrence (Optimistic Locking / Version Mismatch)
    #[error("Concurrency conflict: {reason}")]
    ConcurrencyConflict {
        reason: String
    },

    /// Échec définitif après plusieurs tentatives de retry
    #[error("Operation failed after maximum retries: {0}")]
    TooManyConflicts(String),

    /// Erreur de permissions / business rules (ex: compte banni ou bloqué)
    #[error("Unauthorized access: {reason}")]
    Unauthorized {
        reason: String
    },

    /// Accès interdit malgré une identité valide (RBAC / ABAC)
    #[error("Forbidden: {reason}")]
    Forbidden {
        reason: String
    },

    /// Erreur liée à l'infrastructure (DB, Kafka, Redis)
    #[error("Infrastructure failure: {0}")]
    Infrastructure(String),

    /// Erreur générique du domaine Identity (ex: erreur interne d'agrégat)
    #[error("Internal domain error: {0}")]
    Internal(String),
}

impl DomainError {
    /// Utilisé par la boucle de Retry du Use Case
    pub fn is_concurrency_conflict(&self) -> bool {
        matches!(self, Self::ConcurrencyConflict { .. })
    }

    /// Utilisé pour savoir si l'erreur est fatale et ne doit pas être retry (ex: 409 Conflict sur un Email)
    pub fn is_already_exists(&self) -> bool {
        matches!(self, Self::AlreadyExists { .. })
    }
}

impl From<AppError> for DomainError {
    fn from(err: AppError) -> Self {
        match err.code {
            // Si l'AppError était un Not Found technique (ex: Redis),
            // on peut le transformer en Not Found domaine.
            crate::errors::ErrorCode::NotFound => DomainError::NotFound {
                entity: "Resource",
                id: "unknown".into()
            },
            _ => DomainError::Internal(err.message),
        }
    }
}