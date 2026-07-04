use std::fmt;
use uuid::Uuid;

use crate::error::TimelineError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PostId(Uuid);

impl PostId {
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    pub fn as_uuid(&self) -> Uuid {
        self.0
    }
}

impl TryFrom<&str> for PostId {
    type Error = TimelineError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        Uuid::parse_str(s)
            .map(Self)
            .map_err(|_| TimelineError::InvalidPostId(s.to_owned()))
    }
}

impl fmt::Display for PostId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
