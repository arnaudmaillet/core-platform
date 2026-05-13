use crate::core::Error;

/// Le type Result unique pour tout le Shared Kernel et les contrats partagés.
/// Il utilise l'erreur unifiée qui peut transporter aussi bien du métier que de l'infra.
pub type Result<T> = std::result::Result<T, Error>;
