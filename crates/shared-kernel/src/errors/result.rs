use crate::errors::{AppError, DomainError};

/// RESULT DU DOMAINE (Interne)
/// Utilisé par : Agrégats, Services de domaine, Use Cases, Repositories (Ports).
/// Il force le développeur à traduire les erreurs techniques en erreurs métier.
pub type Result<T> = std::result::Result<T, DomainError>;

/// RESULT D'APPLICATION (Exécutable)
/// Utilisé par : Workers (Outbox), API (Controllers), CLI.
/// Il permet de manipuler des erreurs techniques (Kafka down) et métier simultanément.
pub type AppResult<T> = std::result::Result<T, AppError>;

/// Helper pour les erreurs de type "Internal" rapides
pub fn internal_err(msg: impl Into<String>) -> DomainError {
    DomainError::Internal(msg.into())
}