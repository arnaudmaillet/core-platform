use crate::core::{Error, Identifier, Result, ValueObject};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AccountId(Uuid);

impl AccountId {
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
            return Err(Error::validation("account_id", "Account ID cannot be nil"));
        }
        Ok(())
    }
}

// --- CONVERSIONS ---

impl From<Uuid> for AccountId {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl FromStr for AccountId {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        let uuid = Uuid::parse_str(s).map_err(|_| {
            Error::validation("account_id", format!("'{}' is not a valid UUID string", s))
        })?;
        Ok(Self(uuid))
    }
}

impl fmt::Display for AccountId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl TryFrom<String> for AccountId {
    type Error = Error;

    fn try_from(value: String) -> Result<Self> {
        Self::from_str(&value)
    }
}

impl TryFrom<&str> for AccountId {
    type Error = Error;

    fn try_from(value: &str) -> Result<Self> {
        Self::from_str(value)
    }
}
