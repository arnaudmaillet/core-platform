use std::fmt;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::ProfileError;

/// Opaque, platform-canonical profile identifier (UUIDv7).
///
/// v7 encodes a millisecond-precision timestamp in the top 48 bits, making
/// inserts into ScyllaDB token-aware buckets append-friendly and providing
/// natural time-ordering without a central sequence generator.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProfileId(Uuid);

impl ProfileId {
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }

    pub fn from_uuid(id: Uuid) -> Self {
        Self(id)
    }

    pub fn as_uuid(&self) -> Uuid {
        self.0
    }

    pub fn as_str(&self) -> String {
        self.0.hyphenated().to_string()
    }
}

impl Default for ProfileId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for ProfileId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ProfileId({})", self.0.hyphenated())
    }
}

impl fmt::Display for ProfileId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.hyphenated())
    }
}

impl From<Uuid> for ProfileId {
    fn from(id: Uuid) -> Self {
        Self(id)
    }
}

impl TryFrom<&str> for ProfileId {
    type Error = ProfileError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        Uuid::parse_str(s)
            .map(Self)
            .map_err(|_| ProfileError::InvalidProfileId(s.to_owned()))
    }
}

impl TryFrom<String> for ProfileId {
    type Error = ProfileError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::try_from(s.as_str())
    }
}
