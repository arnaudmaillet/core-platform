use std::fmt;
use uuid::Uuid;

use crate::error::EngagementError;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PostId(Uuid);

impl PostId {
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

impl TryFrom<&str> for PostId {
    type Error = EngagementError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        Uuid::parse_str(s)
            .map(Self)
            .map_err(|_| EngagementError::InvalidPostId(s.to_owned()))
    }
}

impl fmt::Display for PostId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
