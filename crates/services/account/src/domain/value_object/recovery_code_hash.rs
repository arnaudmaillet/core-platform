use std::fmt;

use serde::{Deserialize, Serialize};

/// bcrypt hash of a one-time MFA recovery code.
///
/// Recovery codes are generated as random 16-character base32 strings and
/// immediately hashed with bcrypt before storage. The plaintext is shown to
/// the user exactly once and never stored. This type holds only the hash.
///
/// `Debug` is suppressed to prevent hash leakage into logs.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryCodeHash(String);

impl RecoveryCodeHash {
    pub fn from_hash(hash: impl Into<String>) -> Self {
        Self(hash.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

impl fmt::Debug for RecoveryCodeHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RecoveryCodeHash(\"[redacted]\")")
    }
}
