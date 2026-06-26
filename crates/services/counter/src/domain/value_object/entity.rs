use serde::{Deserialize, Serialize};

use crate::error::CounterError;

/// The kind of entity a counter is attached to. Counter-analytics owns no entity;
/// this is a *reference* discriminant for a record owned by another service
/// (`post` / `profile` / `media` / `comment`, or a hashtag keyed by its tag).
///
/// The `as_str` form is the stable storage discriminant used in Redis keys,
/// ledger rows, and time-series partitions — it is part of the storage contract,
/// so treat changes as a migration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EntityKind {
    Post,
    Profile,
    Media,
    Hashtag,
    Comment,
}

impl EntityKind {
    /// Stable lowercase discriminant used for key/partition naming and event
    /// mapping.
    pub fn as_str(&self) -> &'static str {
        match self {
            EntityKind::Post => "post",
            EntityKind::Profile => "profile",
            EntityKind::Media => "media",
            EntityKind::Hashtag => "hashtag",
            EntityKind::Comment => "comment",
        }
    }

    /// Parse the stable discriminant. An unrecognized value is a contract fault,
    /// surfaced as `CTR-9001 DomainViolation`.
    pub fn try_from_str(s: &str) -> Result<Self, CounterError> {
        match s {
            "post" => Ok(EntityKind::Post),
            "profile" => Ok(EntityKind::Profile),
            "media" => Ok(EntityKind::Media),
            "hashtag" => Ok(EntityKind::Hashtag),
            "comment" => Ok(EntityKind::Comment),
            other => Err(CounterError::DomainViolation {
                field: "entity_type".to_owned(),
                message: format!("unknown entity kind '{other}'"),
            }),
        }
    }
}

/// An opaque entity id owned by the upstream service. Counter never interprets it
/// beyond equality + key composition. A blank id is a malformed event, surfaced
/// as `CTR-9002 InvalidIdentifier`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EntityId(String);

impl EntityId {
    pub fn new(value: impl Into<String>) -> Result<Self, CounterError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(CounterError::InvalidIdentifier("entity_id".to_owned()));
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A reference to the entity a count is attached to: `(kind, id)`. This is the
/// unit counter-analytics aggregates magnitudes *for* — it stores nothing else
/// about the entity.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EntityRef {
    pub kind: EntityKind,
    pub id: EntityId,
}

impl EntityRef {
    pub fn new(kind: EntityKind, id: EntityId) -> Self {
        Self { kind, id }
    }
}

#[cfg(test)]
mod tests {
    use error::AppError;

    use super::*;

    #[test]
    fn entity_kind_round_trips() {
        for kind in [
            EntityKind::Post,
            EntityKind::Profile,
            EntityKind::Media,
            EntityKind::Hashtag,
            EntityKind::Comment,
        ] {
            assert_eq!(EntityKind::try_from_str(kind.as_str()).unwrap(), kind);
        }
    }

    #[test]
    fn entity_kind_rejects_unknown() {
        let err = EntityKind::try_from_str("account").unwrap_err();
        assert_eq!(err.error_code(), "CTR-9001");
    }

    #[test]
    fn entity_id_rejects_blank() {
        let err = EntityId::new("   ").unwrap_err();
        assert_eq!(err.error_code(), "CTR-9002");
    }

    #[test]
    fn entity_id_accepts_non_empty() {
        assert_eq!(EntityId::new("post-1").unwrap().as_str(), "post-1");
    }
}
