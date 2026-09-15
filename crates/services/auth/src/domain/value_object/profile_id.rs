use std::fmt;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::AuthError;

/// A public-persona identifier owned by the `profile` service (a UUIDv7).
///
/// One account owns N profiles, while the edge token's `sub` is the account id.
/// Auth therefore mints the profiles the account owns into the token's `pids`
/// claim so client-facing services can bind a request's profile-keyed actor to
/// the verified caller without a lookup. Auth only references profile ids; it
/// never mints them.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProfileId(Uuid);

impl ProfileId {
    /// Wraps an existing UUID without validation (a verified claim, an upstream
    /// response).
    pub fn from_uuid(id: Uuid) -> Self {
        Self(id)
    }

    pub fn as_uuid(&self) -> Uuid {
        self.0
    }

    /// The hyphenated string representation (the wire form in `pids`).
    pub fn as_str(&self) -> String {
        self.0.hyphenated().to_string()
    }
}

impl fmt::Debug for ProfileId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ProfileId({})", self.0.hyphenated())
    }
}

impl fmt::Display for ProfileId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.hyphenated())
    }
}

impl TryFrom<&str> for ProfileId {
    type Error = AuthError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        Uuid::parse_str(s).map(Self).map_err(|_| AuthError::DomainViolation {
            field: "profile_id".into(),
            message: format!("'{s}' is not a UUID"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_its_string_form() {
        let id = ProfileId::from_uuid(Uuid::now_v7());
        assert_eq!(ProfileId::try_from(id.as_str().as_str()).unwrap(), id);
    }

    #[test]
    fn rejects_non_uuid() {
        assert!(matches!(
            ProfileId::try_from("nope").unwrap_err(),
            AuthError::DomainViolation { .. }
        ));
    }
}
