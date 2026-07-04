use std::fmt;
use uuid::Uuid;

use crate::error::NotificationError;

/// Identifies the entity that was interacted with (a post_id or comment_id).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SubjectId(Uuid);

impl SubjectId {
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

impl TryFrom<&str> for SubjectId {
    type Error = NotificationError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        Uuid::parse_str(s)
            .map(Self)
            .map_err(|_| NotificationError::InvalidSubjectId(s.to_owned()))
    }
}

impl fmt::Display for SubjectId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
