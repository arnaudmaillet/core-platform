use std::fmt;
use std::hash::Hasher;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::AuthError;

/// Internal account identifier the session is bound to.
///
/// This is the platform-canonical account id owned by the `account` service
/// (a UUIDv7) — **not** the IdP subject. The auth context only ever references
/// it; it never mints it. Wrapping it keeps the IdP `sub` (an opaque string
/// living in [`IdpSubject`](super::IdpSubject)) from being confused with the
/// internal id at any call site.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AccountId(Uuid);

impl AccountId {
    /// Wraps an existing UUID without validation. Use when reconstructing from a
    /// trusted source (a DB row, a verified token claim, an upstream response).
    pub fn from_uuid(id: Uuid) -> Self {
        Self(id)
    }

    /// Returns the inner UUID value.
    pub fn as_uuid(&self) -> Uuid {
        self.0
    }

    /// Returns the hyphenated string representation.
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
    type Error = AuthError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        Uuid::parse_str(s)
            .map(Self)
            .map_err(|_| AuthError::InvalidAccountId(s.to_owned()))
    }
}

impl TryFrom<String> for AccountId {
    type Error = AuthError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::try_from(s.as_str())
    }
}

/// `AccountId` is the shard key for every durable auth table (`sessions`,
/// `refresh_tokens`, `subject_links`): all of a person's auth state co-locates,
/// so logout-all and session listing route to a single shard. The 16-byte UUID
/// is fed directly into the hasher — zero allocation.
impl postgres_storage::ShardKey for AccountId {
    #[inline]
    fn hash_shard_key<H: Hasher>(&self, state: &mut H) {
        state.write(self.0.as_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_uuid() {
        let id = AccountId::try_from("018f4c2a-9b7e-7a3d-b2c1-000000000001").unwrap();
        assert_eq!(id.as_str(), "018f4c2a-9b7e-7a3d-b2c1-000000000001");
    }

    #[test]
    fn rejects_garbage() {
        let err = AccountId::try_from("not-a-uuid").unwrap_err();
        assert!(matches!(err, AuthError::InvalidAccountId(_)));
    }

    #[test]
    fn round_trips_through_uuid() {
        let raw = Uuid::now_v7();
        assert_eq!(AccountId::from_uuid(raw).as_uuid(), raw);
    }
}
