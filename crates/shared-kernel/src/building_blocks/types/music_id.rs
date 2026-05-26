// crates/post/src/domain/types/music_id.rs

use crate::core::{Error, Identifier, Result, ValueObject};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use uuid::Uuid;

/// Identifiant unique fort pour une piste audio du catalogue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct MusicId(Uuid);

impl MusicId {
    /// Crée un MusicId à partir d'un UUID existant
    pub fn new(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Génère un nouveau MusicId (Utilise UUID v4 ou v7 selon les standards de ton catalogue audio)
    pub fn generate() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn uuid(&self) -> Uuid {
        self.0
    }
}

impl Identifier for MusicId {
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

impl ValueObject for MusicId {
    fn validate(&self) -> Result<()> {
        if self.0.is_nil() {
            return Err(Error::validation("music_id", "Music UUID cannot be nil"));
        }
        Ok(())
    }
}

// --- CONVERSIONS ---

impl FromStr for MusicId {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self> {
        let uuid = Uuid::parse_str(s).map_err(|_| {
            Error::validation("music_id", format!("'{}' is not a valid Music UUID", s))
        })?;
        Ok(Self(uuid))
    }
}

impl fmt::Display for MusicId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl TryFrom<String> for MusicId {
    type Error = Error;
    fn try_from(value: String) -> Result<Self> {
        Self::from_str(&value)
    }
}
