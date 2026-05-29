// crates/post/src/domain/types/music_id.rs

use crate::core::{Error, Identifier, Result, ValueObject};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct MusicId(Uuid);

impl MusicId {
    pub fn new(uuid: Uuid) -> Self {
        Self(uuid)
    }

    pub fn generate() -> Self {
        Self(Uuid::now_v7())
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
