// crates/shared-kernel/src/domain/value_object/account_id.rs

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use uuid::Uuid;
use crate::domain::entities::EntityMetadata;
use crate::domain::value_objects::ValueObject;
use crate::domain::identifier::Identifier; // Nouvel import
use crate::errors::{DomainError, Result};

/// Identifiant unique pour un compte, basé sur UUID v7 (triable temporellement).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct AccountId(Uuid);

impl AccountId {
    /// Génère un nouvel identifiant unique (UUID v7).
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }

    /// Crée une instance à partir d'une chaîne de caractères avec validation.
    pub fn try_new(id: impl Into<String>) -> Result<Self> {
        Self::from_str(&id.into())
    }
}

// Implémentation du contrat d'identité générique
impl Identifier for AccountId {
    fn as_uuid(&self) -> Uuid {
        self.0
    }

    fn as_string(&self) -> String {
        self.0.to_string()
    }

    fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl ValueObject for AccountId {
    fn validate(&self) -> Result<()> {
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

impl From<Uuid> for AccountId {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl TryFrom<String> for AccountId {
    type Error = DomainError;
    fn try_from(value: String) -> Result<Self> {
        Self::from_str(&value)
    }
}

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