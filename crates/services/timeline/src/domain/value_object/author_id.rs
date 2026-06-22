use std::fmt;
use uuid::Uuid;

use crate::error::TimelineError;

/// Identifies a post author (producer) in the feed engine.
///
/// Semantically distinct from `ProfileId` (a feed consumer) even though both
/// wrap a UUID. An author owns `posts_by_author` partitions and VIP registries;
/// a profile owns `feed_items_by_profile` partitions and feed ZSETs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AuthorId(Uuid);

impl AuthorId {
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    pub fn as_uuid(&self) -> Uuid {
        self.0
    }
}

impl TryFrom<&str> for AuthorId {
    type Error = TimelineError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        Uuid::parse_str(s)
            .map(Self)
            .map_err(|_| TimelineError::InvalidAuthorId(s.to_owned()))
    }
}

impl fmt::Display for AuthorId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
