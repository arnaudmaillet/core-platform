//! Identifier value objects for the media domain.
//!
//! [`AssetId`] is a freshly-minted **UUIDv7** (time-ordered) — an asset is
//! reserved exactly once at ticket time, so there is no dedup-by-id requirement
//! here (content-addressed dedup happens by [`ContentHash`](super::ContentHash),
//! not by id). [`OwnerId`] wraps the account UUID the asset belongs to and the
//! events are partitioned alongside.

use std::fmt;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::MediaError;

/// Opaque asset identifier (UUIDv7, time-ordered). Minted once when the upload
/// ticket is issued.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AssetId(Uuid);

impl AssetId {
    /// Generates a fresh UUIDv7 identifier.
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }

    /// Wraps an existing UUID from a trusted source (storage row).
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

impl Default for AssetId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for AssetId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "AssetId({})", self.0.hyphenated())
    }
}

impl fmt::Display for AssetId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.hyphenated())
    }
}

impl TryFrom<&str> for AssetId {
    type Error = MediaError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        Uuid::parse_str(s)
            .map(Self)
            .map_err(|_| MediaError::InvalidIdentifier(s.to_owned()))
    }
}

/// The account that owns an asset (its authorization principal and the events'
/// secondary routing key). Backed by a UUID to match the rest of the fleet.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct OwnerId(Uuid);

impl OwnerId {
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

impl fmt::Debug for OwnerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "OwnerId({})", self.0.hyphenated())
    }
}

impl fmt::Display for OwnerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.hyphenated())
    }
}

impl TryFrom<&str> for OwnerId {
    type Error = MediaError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        Uuid::parse_str(s)
            .map(Self)
            .map_err(|_| MediaError::InvalidIdentifier(s.to_owned()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asset_id_is_v7_and_unique() {
        let a = AssetId::new();
        let b = AssetId::new();
        assert_ne!(a, b);
        assert_eq!(a.as_uuid().get_version_num(), 7);
    }

    #[test]
    fn ids_round_trip_through_string() {
        let a = AssetId::new();
        assert_eq!(AssetId::try_from(a.as_str().as_str()).unwrap(), a);
        let o = OwnerId::from_uuid(Uuid::from_u128(42));
        assert_eq!(OwnerId::try_from(o.as_str().as_str()).unwrap(), o);
    }

    #[test]
    fn id_parse_rejects_garbage() {
        assert!(matches!(
            AssetId::try_from("not-a-uuid").unwrap_err(),
            MediaError::InvalidIdentifier(_)
        ));
        assert!(matches!(
            OwnerId::try_from("nope").unwrap_err(),
            MediaError::InvalidIdentifier(_)
        ));
    }
}
