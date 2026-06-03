// crates/post/src/domain/types/post_id.rs

use crate::core::{Error, Identifier, Result, ValueObject};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use uuid::Uuid;

/// Identifiant unique chronologique (UUIDv7) pour une publication.
/// Extrait nativement sa date de création depuis ses 48 premiers bits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct PostId(Uuid);

impl PostId {
    /// Crée un PostId à partir d'un UUID existant
    pub fn new(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Génère un nouvel identifiant unique basé sur le standard UUIDv7 (Horodaté)
    pub fn generate() -> Self {
        Self(Uuid::now_v7())
    }

    /// Accesseur pour l'UUID brut (indispensable pour les drivers d'infrastructure)
    pub fn uuid(&self) -> Uuid {
        self.0
    }

    /// Extrait instantanément la date de création (`DateTime<Utc>`) encapsulée dans l'UUIDv7.
    /// Évite de stocker une colonne physique `created_at` dans ScyllaDB.
    pub fn created_at(&self) -> DateTime<Utc> {
        let bytes = self.0.into_bytes();

        // L'UUIDv7 stocke le timestamp UNIX en millisecondes sur ses 48 premiers bits (octets 0 à 5)
        let mut ts_ms: u64 = 0;
        for i in 0..6 {
            ts_ms = (ts_ms << 8) | bytes[i] as u64;
        }

        // Conversion en DateTime<Utc> sécurisée
        DateTime::from_timestamp_millis(ts_ms as i64)
            .expect("PostId contains an invalid or corrupted UUIDv7 timestamp")
    }
}

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

    fn identifier_scope() -> &'static str {
        "post"
    }
}

impl ValueObject for PostId {
    fn validate(&self) -> Result<()> {
        if self.0.is_nil() {
            return Err(Error::validation("post_id", "Post UUID cannot be nil"));
        }

        // Validation stricte : On s'assure que l'UUID fourni est bien un UUIDv7 (version 7)
        if self.0.get_version_num() != 7 {
            return Err(Error::validation(
                "post_id",
                "Invalid PostId format. PostId must strictly be a time-sortable UUIDv7",
            ));
        }

        Ok(())
    }
}

// --- CONVERSIONS ---

impl FromStr for PostId {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self> {
        let uuid = Uuid::parse_str(s).map_err(|_| {
            Error::validation("post_id", format!("'{}' is not a valid UUID string", s))
        })?;

        let post_id = Self(uuid);
        post_id.validate()?;
        Ok(post_id)
    }
}

impl fmt::Display for PostId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl TryFrom<String> for PostId {
    type Error = Error;
    fn try_from(value: String) -> Result<Self> {
        Self::from_str(&value)
    }
}

impl TryFrom<&str> for PostId {
    type Error = Error;
    fn try_from(value: &str) -> Result<Self> {
        Self::from_str(value)
    }
}
