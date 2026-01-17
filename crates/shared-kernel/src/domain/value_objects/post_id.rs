// crates/shared-kernel/src/domain/post_id.rs

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use uuid::Uuid;
use crate::domain::entities::EntityMetadata;
use crate::domain::value_objects::ValueObject;
use crate::errors::{DomainError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct PostId(Uuid);

impl PostId {
    /// Génère un nouvel UUID v7 (Temporellement ordonné)
    /// Idéal pour les ressources à haut débit comme les Posts.
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }

    /// Reconstruction sans validation (usage interne/mapping DB)
    pub fn new_unchecked(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Validation et création depuis une String (Entrée API)
    pub fn try_new(id: impl Into<String>) -> Result<Self> {
        let s = id.into();
        Self::from_str(&s)
    }

    pub fn as_uuid(&self) -> Uuid {
        self.0
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

impl fmt::Display for PostId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl EntityMetadata for PostId {
    fn entity_name() -> &'static str {
        "PostId"
    }
}