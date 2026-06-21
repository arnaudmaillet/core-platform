use std::fmt;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::ProfileError;

/// Local mirror of the account service's AccountId value object.
///
/// Defined here instead of importing from `services/account` to maintain strict
/// service boundary isolation. The wire format (UUIDv7 hyphenated string) is
/// identical — callers that receive an account_id from an external source
/// (gRPC, Kafka event) parse it through this type.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AccountId(Uuid);

impl AccountId {
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

impl fmt::Debug for AccountId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "AccountId({})", self.0.hyphenated())
    }
}

impl fmt::Display for AccountId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.hyphenated())
    }
}

impl From<Uuid> for AccountId {
    fn from(id: Uuid) -> Self {
        Self(id)
    }
}

impl TryFrom<&str> for AccountId {
    type Error = ProfileError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        Uuid::parse_str(s)
            .map(Self)
            .map_err(|_| ProfileError::DomainViolation {
                field: "account_id".into(),
                message: format!("invalid UUID: '{s}'"),
            })
    }
}

impl TryFrom<String> for AccountId {
    type Error = ProfileError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::try_from(s.as_str())
    }
}
