use crate::core::ErrorCode;
use serde::Serialize;
use serde_json::{Value, json};
use std::fmt;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Serialize, Clone)]
pub struct Error {
    pub code: ErrorCode,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
    #[serde(skip)]
    pub source: Option<String>,
}

impl Error {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            details: None,
            source: None,
        }
    }

    /// Ajoute des détails structurés (ex: erreurs de validation par champ)
    pub fn with_details(mut self, details: Value) -> Self {
        self.details = Some(details);
        self
    }

    /// Enregistre l'erreur technique d'origine pour le debugging interne
    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }

    // --- ERREURS "DOMAINE" (Ex-DomainError) ---

    pub fn validation(field: &'static str, reason: impl Into<String>) -> Self {
        let reason_str = reason.into();
        Self::new(
            ErrorCode::ValidationFailed,
            format!("Validation failed for {field}"),
        )
        .with_details(json!({ "field": field, "reason": reason_str }))
    }

    pub fn not_found(entity: &'static str, id: impl Into<String>) -> Self {
        Self::new(
            ErrorCode::NotFound,
            format!("{entity} with id '{}' not found", id.into()),
        )
    }

    pub fn already_exists(
        entity: &'static str,
        field: &'static str,
        value: impl Into<String>,
    ) -> Self {
        Self::new(
            ErrorCode::AlreadyExists,
            format!("{entity} with {field} '{}' already exists", value.into()),
        )
    }

    pub fn concurrency_conflict(reason: impl Into<String>) -> Self {
        Self::new(ErrorCode::ConcurrencyConflict, reason)
    }

    pub fn unauthorized(reason: impl Into<String>) -> Self {
        Self::new(ErrorCode::Unauthorized, reason)
    }

    pub fn forbidden(reason: impl Into<String>) -> Self {
        Self::new(ErrorCode::Forbidden, reason)
    }

    pub fn precondition_failed(reason: impl Into<String>) -> Self {
        Self::new(ErrorCode::PreconditionFailed, reason)
    }
    pub fn max_retries_exceeded(attempts: u32, source_error: impl Into<String>) -> Self {
        Self::new(
            ErrorCode::MaxRetriesExceeded,
            format!(
                "Operation failed after {} attempts due to persistent conflicts",
                attempts
            ),
        )
        .with_source(source_error)
    }

    // --- ERREURS "TECHNIQUES" (Ex-InfrastructureError) ---

    pub fn internal(msg: impl Into<String>) -> Self {
        let msg_str = msg.into();
        // On log l'erreur réelle ici pour ne jamais la perdre
        tracing::error!("Internal System Error: {}", msg_str);

        Self::new(
            ErrorCode::InternalError,
            "An internal server error occurred",
        )
        .with_source(msg_str)
    }

    pub fn database(source: impl Into<String>) -> Self {
        let source_str = source.into();
        tracing::error!("Database operation failed. Raw source: {:#?}", source_str);

        Self::new(
            ErrorCode::InfrastructureFailure,
            "Database operation failed",
        )
        .with_source(source_str)
    }

    pub fn messaging(source: impl Into<String>) -> Self {
        Self::new(
            ErrorCode::InfrastructureFailure,
            "Message broker communication failed",
        )
        .with_source(source)
    }
    pub fn source(&self) -> Option<&str> {
        self.source.as_deref()
    }
}

// --- CONVERSIONS AUTOMATIQUES (Traits From) ---

#[cfg(feature = "postgres")]
impl From<sqlx::Error> for Error {
    fn from(err: sqlx::Error) -> Self {
        match err {
            sqlx::Error::RowNotFound => Self::not_found("Database Row", "unknown"),
            _ => Self::database(err.to_string()),
        }
    }
}

#[cfg(feature = "kafka")]
impl From<rdkafka::error::KafkaError> for Error {
    fn from(err: rdkafka::error::KafkaError) -> Self {
        Self::messaging(err.to_string())
    }
}

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Self::new(ErrorCode::InternalError, "Serialization failed").with_source(err.to_string())
    }
}

// --- IMPLEMENTATIONS STANDARD ---

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{:?}] {}", self.code, self.message)
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }
}
