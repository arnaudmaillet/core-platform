use std::fmt;

use uuid::Uuid;

use crate::error::GeoDiscoveryError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AuthorId(Uuid);

impl AuthorId {
    pub fn as_uuid(&self) -> Uuid {
        self.0
    }
}

impl fmt::Display for AuthorId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl TryFrom<&str> for AuthorId {
    type Error = GeoDiscoveryError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        Uuid::parse_str(s)
            .map(Self)
            .map_err(|_| GeoDiscoveryError::InvalidAuthorId(s.to_owned()))
    }
}

impl From<Uuid> for AuthorId {
    fn from(id: Uuid) -> Self {
        Self(id)
    }
}
