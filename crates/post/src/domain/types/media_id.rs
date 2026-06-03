// crates/post/src/domain/types/media_id.rs

use serde::{Deserialize, Serialize};
use shared_kernel::core::{Error, Identifier, Result, ValueObject};
use std::fmt;
use std::str::FromStr;
use uuid::Uuid;

/// Identifiant unique fort (UUIDv4) pour un asset média individuel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct MediaId(Uuid);

impl MediaId {
    pub fn new(uuid: Uuid) -> Self {
        Self(uuid)
    }

    pub fn generate() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn uuid(&self) -> Uuid {
        self.0
    }
}

impl Identifier for MediaId {
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
        "media"
    }
}

impl ValueObject for MediaId {
    fn validate(&self) -> Result<()> {
        if self.0.is_nil() {
            return Err(Error::validation("media_id", "Media UUID cannot be nil"));
        }

        if self.0.get_version_num() != 4 {
            return Err(Error::validation(
                "media_id",
                "Invalid MediaId format. MediaId must strictly be an UUIDv4 (Random)",
            ));
        }

        Ok(())
    }
}

impl FromStr for MediaId {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self> {
        let uuid = Uuid::parse_str(s).map_err(|_| {
            Error::validation("media_id", format!("'{}' is not a valid UUID string", s))
        })?;

        let media_id = Self(uuid);
        media_id.validate()?;
        Ok(media_id)
    }
}

impl fmt::Display for MediaId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl TryFrom<String> for MediaId {
    type Error = Error;
    fn try_from(value: String) -> Result<Self> {
        Self::from_str(&value)
    }
}

impl TryFrom<&str> for MediaId {
    type Error = Error;
    fn try_from(value: &str) -> Result<Self> {
        Self::from_str(value)
    }
}
