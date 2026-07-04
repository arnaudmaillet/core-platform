use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::SocialGraphError;

/// Opaque identifier for a profile within the social-graph bounded context.
///
/// This service treats profiles as pure UUIDv7 primitives. It has no knowledge
/// of handles, display names, or any other metadata — those are the responsibility
/// of services/profile. SRP is maintained by never importing the profile crate here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProfileId(Uuid);

impl ProfileId {
    pub fn from_uuid(id: Uuid) -> Self {
        Self(id)
    }

    pub fn as_uuid(&self) -> Uuid {
        self.0
    }

    pub fn as_str(&self) -> String {
        self.0.to_string()
    }
}

impl TryFrom<&str> for ProfileId {
    type Error = SocialGraphError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        Uuid::parse_str(s)
            .map(Self)
            .map_err(|_| SocialGraphError::InvalidProfileId(s.to_owned()))
    }
}

impl std::fmt::Display for ProfileId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
