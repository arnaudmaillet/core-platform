use serde::{Deserialize, Serialize};

use crate::error::SearchError;

/// The kind of entity a search document refers to. Each kind lives in its own
/// physical index (its own analyzer chain); a federated query fans out across the
/// requested kinds. The `as_str` form is the stable index-name discriminant — it
/// is part of the storage contract, so treat changes as a reindex.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EntityKind {
    Profile,
    Post,
    Hashtag,
}

impl EntityKind {
    /// Stable lowercase discriminant used for index naming and event mapping.
    pub fn as_str(&self) -> &'static str {
        match self {
            EntityKind::Profile => "profile",
            EntityKind::Post => "post",
            EntityKind::Hashtag => "hashtag",
        }
    }

    /// Parse the stable discriminant. An unrecognized value is a contract fault,
    /// surfaced as `SCH-1003 UnsupportedEntityType`.
    pub fn try_from_str(s: &str) -> Result<Self, SearchError> {
        match s {
            "profile" => Ok(EntityKind::Profile),
            "post" => Ok(EntityKind::Post),
            "hashtag" => Ok(EntityKind::Hashtag),
            other => Err(SearchError::UnsupportedEntityType {
                entity_type: other.to_owned(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use error::AppError;

    use super::*;

    #[test]
    fn round_trips_through_str() {
        for kind in [EntityKind::Profile, EntityKind::Post, EntityKind::Hashtag] {
            assert_eq!(EntityKind::try_from_str(kind.as_str()).unwrap(), kind);
        }
    }

    #[test]
    fn rejects_unknown_kind() {
        let err = EntityKind::try_from_str("account").unwrap_err();
        assert_eq!(err.error_code(), "SCH-1003");
    }
}
