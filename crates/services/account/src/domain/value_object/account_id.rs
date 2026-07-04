use std::fmt;
use std::hash::Hasher;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::AccountError;

/// Opaque, platform-canonical account identifier.
///
/// Wraps a UUIDv7, which encodes a millisecond-precision timestamp in its
/// top 48 bits. This makes inserts into a B-tree index append-friendly (no
/// hot-page contention) while retaining global uniqueness without a central
/// sequence generator — a critical property for a distributed cluster.
///
/// The inner `Uuid` is private: callers must use the provided constructors
/// and accessors, which prevents accidental misuse of the raw bytes.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AccountId(Uuid);

impl AccountId {
    /// Generates a fresh UUIDv7 account identifier.
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }

    /// Wraps an existing UUID without validation.
    ///
    /// Prefer [`AccountId::new`] for creation. Use this only when
    /// reconstructing an `AccountId` from a trusted source (e.g. a database
    /// row or a deserialised payload).
    pub fn from_uuid(id: Uuid) -> Self {
        Self(id)
    }

    /// Returns the inner UUID value.
    pub fn as_uuid(&self) -> Uuid {
        self.0
    }

    /// Returns the hyphenated string representation, e.g.
    /// `"018f4c2a-9b7e-7a3d-b2c1-000000000001"`.
    pub fn as_str(&self) -> String {
        self.0.hyphenated().to_string()
    }
}

impl Default for AccountId {
    fn default() -> Self {
        Self::new()
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
    type Error = AccountError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        Uuid::parse_str(s)
            .map(Self)
            .map_err(|_| AccountError::InvalidAccountId(s.to_owned()))
    }
}

impl TryFrom<String> for AccountId {
    type Error = AccountError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::try_from(s.as_str())
    }
}

/// `AccountId` is the canonical shard key for the account bounded context.
///
/// The 16-byte UUID is fed directly into the hasher — zero allocation, no
/// intermediate string representation. The `ShardKey` trait lives in the
/// `postgres-storage` crate, but it carries no I/O semantics: it is purely
/// a `std::hash::Hasher`-feed contract. Implementing it here avoids wrapper
/// types or an extra adapter layer at every repository call site.
impl postgres_storage::ShardKey for AccountId {
    #[inline]
    fn hash_shard_key<H: Hasher>(&self, state: &mut H) {
        state.write(self.0.as_bytes());
    }
}
