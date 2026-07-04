use std::fmt;

use serde::{Deserialize, Serialize};

/// Opaque Argon2id password hash.
///
/// # Secret protection
///
/// Both `Debug` and `Display` are **intentionally suppressed**: the manual
/// `Debug` impl prints `"[redacted]"`, and `Display` is not implemented at
/// all. This guarantees that the hash cannot leak into log lines, error
/// messages, or distributed traces regardless of how the type is used.
///
/// `Serialize` is provided so the repository adapter can persist the hash
/// to the database. It must never be included in API responses — mappers
/// in the infrastructure layer must actively exclude it.
#[derive(Clone, Serialize, Deserialize)]
pub struct PasswordHash(String);

impl PasswordHash {
    /// Wraps a pre-computed Argon2id hash string.
    ///
    /// No validation is performed — the caller is responsible for ensuring
    /// the input is a well-formed Argon2id PHC string. Validation at
    /// construction time would add a dependency on the hashing library to
    /// the domain layer, which must remain infrastructure-free.
    pub fn from_hash(hash: impl Into<String>) -> Self {
        Self(hash.into())
    }

    /// Returns the raw hash string for use by the repository adapter.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

impl fmt::Debug for PasswordHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PasswordHash(\"[redacted]\")")
    }
}

// `Display` is intentionally NOT implemented. Any attempt to format a
// `PasswordHash` as a plain string will produce a compile error, making
// accidental exposure impossible at the type level.
