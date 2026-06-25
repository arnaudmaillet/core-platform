use std::fmt;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::AuthError;

/// Opaque session identifier (UUIDv7).
///
/// Travels in the edge token's `sid` claim so a downstream service — or this
/// service's `Introspect` — can name the exact session a request belongs to.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SessionId(Uuid);

impl SessionId {
    /// Generates a fresh UUIDv7 session identifier.
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }

    /// Wraps an existing UUID without validation (reconstruction from storage).
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

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SessionId({})", self.0.hyphenated())
    }
}

impl fmt::Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.hyphenated())
    }
}

impl TryFrom<&str> for SessionId {
    type Error = AuthError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        Uuid::parse_str(s)
            .map(Self)
            .map_err(|_| AuthError::InvalidSessionId(s.to_owned()))
    }
}

impl TryFrom<String> for SessionId {
    type Error = AuthError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::try_from(s.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_is_unique_and_v7() {
        let a = SessionId::new();
        let b = SessionId::new();
        assert_ne!(a, b);
        assert_eq!(a.as_uuid().get_version_num(), 7);
    }

    #[test]
    fn rejects_garbage() {
        assert!(matches!(
            SessionId::try_from("nope").unwrap_err(),
            AuthError::InvalidSessionId(_)
        ));
    }
}
