use std::fmt;
use uuid::Uuid;
use crate::error::PostError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AudioId(Uuid);

impl AudioId {
    pub fn new_v7() -> Self {
        Self(Uuid::now_v7())
    }

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

impl TryFrom<&str> for AudioId {
    type Error = PostError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        Uuid::parse_str(s)
            .map(Self)
            .map_err(|_| PostError::InvalidAudioId(s.to_owned()))
    }
}

impl fmt::Display for AudioId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
