// crates/shared-kernel/src/domain/post_id.rs

use crate::domain::Identifier;
use crate::domain::entities::EntityMetadata;
use crate::domain::value_objects::ValueObject;
use crate::errors::{DomainError, Result};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct PostId(Uuid);

impl PostId {
    /// Génère un nouvel UUID v7.
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }

    /// Validation et création depuis une String (Entrée API).
    pub fn try_new(id: impl Into<String>) -> Result<Self> {
        Self::from_str(&id.into())
    }
}

// Implémentation du trait Identifier pour la généricité
impl Identifier for PostId {
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

impl ValueObject for PostId {
    fn validate(&self) -> Result<()> {
        if self.0.is_nil() {
            return Err(DomainError::Validation {
                field: "post_id",
                reason: "Post ID cannot be nil".to_string(),
            });
        }
        Ok(())
    }
}

impl Default for PostId {
    fn default() -> Self {
        Self::new()
    }
}

// --- CONVERSIONS ---

impl From<Uuid> for PostId {
    fn from(uuid: Uuid) -> Self {
        Self::from_uuid(uuid)
    }
}

impl FromStr for PostId {
    type Err = DomainError;
    fn from_str(s: &str) -> Result<Self> {
        Uuid::parse_str(s)
            .map(Self)
            .map_err(|_| DomainError::Validation {
                field: "post_id",
                reason: format!("'{}' is not a valid UUID for a post", s),
            })
    }
}

impl TryFrom<String> for PostId {
    type Error = DomainError;

    fn try_from(s: String) -> Result<Self> {
        Self::from_str(&s)
    }
}

// Optionnel mais pratique : TryFrom<&str>
impl TryFrom<&str> for PostId {
    type Error = DomainError;

    fn try_from(s: &str) -> Result<Self> {
        Self::from_str(s)
    }
}

impl fmt::Display for PostId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl EntityMetadata for PostId {
    fn entity_name() -> &'static str {
        "Post" // Souvent raccourci pour les messages d'erreur
    }
}
