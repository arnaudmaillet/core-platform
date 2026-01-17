// crates/shared-kernel/src/domain/value_object/account_id

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use uuid::Uuid;
use crate::domain::entities::EntityMetadata;
use crate::domain::value_objects::ValueObject;
use crate::errors::{DomainError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct AccountId(Uuid);

impl AccountId {
    /// Génère un nouvel UUID v7 (Séquentiel, optimisé pour les index DB)
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }

    /// Reconstruction depuis un type sûr (Interne/DB)
    pub fn new_unchecked(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Validation et création depuis une String (API/Entrée externe)
    pub fn try_new(id: impl Into<String>) -> Result<Self> {
        let s = id.into();
        Self::from_str(&s)
    }

    pub fn as_uuid(&self) -> Uuid {
        self.0
    }
}

impl ValueObject for AccountId {
    fn validate(&self) -> Result<()> {
        // Un UUID est techniquement toujours valide s'il est parsé.
        // On pourrait vérifier ici s'il n'est pas "nil" (0000...) si le métier l'interdit.
        if self.0.is_nil() {
            return Err(DomainError::Validation {
                field: "account_id",
                reason: "Account ID cannot be nil".to_string(),
            });
        }
        Ok(())
    }
}

impl Default for AccountId {
    fn default() -> Self {
        Self::new()
    }
}

// --- CONVERSIONS ---

impl FromStr for AccountId {
    type Err = DomainError;
    fn from_str(s: &str) -> Result<Self> {
        Uuid::parse_str(s)
            .map(Self)
            .map_err(|_| DomainError::Validation {
                field: "account_id",
                reason: format!("'{}' is not a valid UUID", s),
            })
    }
}

impl fmt::Display for AccountId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl EntityMetadata for AccountId {
    fn entity_name() -> &'static str {
        "AccountId"
    }
}