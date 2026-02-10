// crates/profile/src/domain/value_objects/profile_id.rs

use shared_kernel::domain::entities::EntityMetadata;
use shared_kernel::domain::value_objects::ValueObject;
use shared_kernel::domain::Identifier;
use shared_kernel::errors::{DomainError, Result};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use uuid::Uuid;


/// Identifiant unique pour un profil, basé sur UUID v7.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProfileId(Uuid);

impl ProfileId {
    /// Génère un nouvel identifiant unique (UUID v7).
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }

    /// Crée une instance à partir d'une chaîne de caractères avec validation.
    pub fn try_new(id: impl Into<String>) -> Result<Self> {
        Self::from_str(&id.into())
    }
}

// Implémentation du contrat d'identité générique du Shared Kernel
impl Identifier for ProfileId {
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

impl ValueObject for ProfileId {
    fn validate(&self) -> Result<()> {
        if self.0.is_nil() {
            return Err(DomainError::Validation {
                field: "profile_id",
                reason: "Profile ID cannot be nil".to_string(),
            });
        }
        Ok(())
    }
}

impl Default for ProfileId {
    fn default() -> Self {
        Self::new()
    }
}

// --- CONVERSIONS ---

impl From<Uuid> for ProfileId {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl TryFrom<String> for ProfileId {
    type Error = DomainError;
    fn try_from(value: String) -> Result<Self> {
        Self::from_str(&value)
    }
}

impl FromStr for ProfileId {
    type Err = DomainError;
    fn from_str(s: &str) -> Result<Self> {
        Uuid::parse_str(s)
            .map(Self)
            .map_err(|_| DomainError::Validation {
                field: "profile_id",
                reason: format!("'{}' is not a valid UUID for ProfileId", s),
            })
    }
}

impl fmt::Display for ProfileId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl EntityMetadata for ProfileId {
    fn entity_name() -> &'static str {
        "ProfileId"
    }
}