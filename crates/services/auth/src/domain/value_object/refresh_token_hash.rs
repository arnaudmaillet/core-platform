use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::AuthError;

/// The stored, irreversible hash of an opaque refresh token.
///
/// The plaintext refresh token is a 256-bit random handle shown to the client
/// exactly once; only this hash is persisted, so a database disclosure never
/// yields usable tokens. The domain treats the value opaquely — the choice of
/// digest (SHA-256, Argon2id, …) is an infrastructure concern. A lookup matches
/// by recomputing the presented token's hash and comparing.
#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RefreshTokenHash(String);

impl RefreshTokenHash {
    /// Wraps a precomputed hash string, rejecting an empty value.
    pub fn new(hash: impl Into<String>) -> Result<Self, AuthError> {
        let hash = hash.into();
        if hash.trim().is_empty() {
            return Err(AuthError::DomainViolation {
                field: "refresh_token_hash".into(),
                message: "hash must not be empty".into(),
            });
        }
        Ok(Self(hash))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Never print the hash itself, even at debug — keep it out of logs entirely.
impl fmt::Debug for RefreshTokenHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RefreshTokenHash(***)")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty() {
        assert!(RefreshTokenHash::new("  ").is_err());
    }

    #[test]
    fn debug_is_redacted() {
        let h = RefreshTokenHash::new("deadbeef").unwrap();
        assert_eq!(format!("{h:?}"), "RefreshTokenHash(***)");
        assert_eq!(h.as_str(), "deadbeef");
    }
}
