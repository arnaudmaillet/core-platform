use std::fmt;
use uuid::Uuid;

use crate::error::ChatError;

/// Identity of a platform profile participating in a conversation, in any role
/// (owner, admin, member, subscriber, or guest). Mirrors the `ProfileId` used
/// across the other services so cross-service references stay uniform.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProfileId(Uuid);

impl ProfileId {
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    pub fn as_uuid(&self) -> Uuid {
        self.0
    }

    pub fn as_str(&self) -> String {
        self.0.to_string()
    }
}

impl TryFrom<&str> for ProfileId {
    type Error = ChatError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        Uuid::parse_str(s)
            .map(Self)
            .map_err(|_| ChatError::InvalidProfileId(s.to_owned()))
    }
}

impl fmt::Display for ProfileId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
