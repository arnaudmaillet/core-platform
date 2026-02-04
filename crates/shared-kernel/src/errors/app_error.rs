use crate::errors::{DomainError, ErrorCode};
use serde::Serialize;
use serde_json::Value;
use std::fmt;

#[derive(Debug, Serialize, Clone)]
pub struct AppError {
    pub code: ErrorCode,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
}

impl AppError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            details: None,
        }
    }

    pub fn with_details(mut self, details: Value) -> Self {
        self.details = Some(details);
        self
    }
}

impl From<DomainError> for AppError {
    fn from(error: DomainError) -> Self {
        match error {
            // 1. Cas : Entité introuvable (404)
            DomainError::NotFound { entity, id } => Self::new(
                ErrorCode::NotFound,
                format!("{entity} with id '{id}' not found"),
            ),

            // 2. Cas : Conflit d'unicité (409) - ex: Email déjà pris
            DomainError::AlreadyExists {
                entity,
                field,
                value,
            } => Self::new(
                ErrorCode::AlreadyExists,
                format!("{entity} with {field} '{value}' already exists"),
            ),

            // 3. Cas : Concurrence (409/429) - Retry géré par le Use Case
            DomainError::ConcurrencyConflict { reason } => {
                Self::new(ErrorCode::ConcurrencyConflict, reason)
            }

            // 4. Cas : Validation (400)
            DomainError::Validation { field, reason } => Self {
                code: ErrorCode::ValidationFailed,
                message: format!("Validation failed for {field}"),
                details: Some(serde_json::json!({ "field": field, "reason": reason })),
            },

            // 5. Cas : Identité non valide (401)
            DomainError::Unauthorized { reason } => Self::new(ErrorCode::Unauthorized, reason),

            // 6. Cas : Droits insuffisants (403)
            DomainError::Forbidden { reason } => Self::new(ErrorCode::Forbidden, reason),

            // 7. Cas : Erreurs techniques (500)
            // On utilise (_) car ce sont des variantes Tuples et on masque le détail au client
            DomainError::Infrastructure(_)
            | DomainError::Internal(_)
            | DomainError::TooManyConflicts(_) => Self::new(
                ErrorCode::InternalError,
                "An unexpected error occurred. Please try again later.",
            ),
        }
    }
}

// Pour transformer les erreurs SQL (sqlx) en AppError
#[cfg(feature = "postgres")]
impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        // En interne, on log l'erreur réelle pour le debugging
        tracing::error!("Database infrastructure error: {:?}", err);

        Self::new(ErrorCode::InternalError, "A database error occurred")
    }
}

// Pour transformer les erreurs Kafka (rdkafka) en AppError
// Note: rdkafka utilise souvent KafkaError
#[cfg(feature = "kafka")]
impl From<rdkafka::error::KafkaError> for AppError {
    fn from(err: rdkafka::error::KafkaError) -> Self {
        tracing::error!("Kafka infrastructure error: {:?}", err);

        Self::new(
            ErrorCode::InternalError,
            format!("Messaging system error: {}", err),
        )
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{:?}] {}", self.code, self.message)
    }
}

impl std::error::Error for AppError {}
