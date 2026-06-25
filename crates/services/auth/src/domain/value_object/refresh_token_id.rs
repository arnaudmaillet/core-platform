use std::fmt;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::AuthError;

/// Identifier of a single refresh-token row (UUIDv7).
///
/// Each rotation mints a new `RefreshTokenId`; the chain of `replaced_by` links
/// forms the rotation lineage used for reuse-detection.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RefreshTokenId(Uuid);

impl RefreshTokenId {
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

impl Default for RefreshTokenId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for RefreshTokenId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RefreshTokenId({})", self.0.hyphenated())
    }
}

impl fmt::Display for RefreshTokenId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.hyphenated())
    }
}

impl TryFrom<&str> for RefreshTokenId {
    type Error = AuthError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        Uuid::parse_str(s)
            .map(Self)
            .map_err(|_| AuthError::DomainViolation {
                field: "refresh_token_id".into(),
                message: format!("invalid UUID: '{s}'"),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_is_unique() {
        assert_ne!(RefreshTokenId::new(), RefreshTokenId::new());
    }
}
