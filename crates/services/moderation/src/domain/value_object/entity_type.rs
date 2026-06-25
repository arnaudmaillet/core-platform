use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::ModerationError;

/// The kind of entity a moderation subject points at. Moderation references
/// content by `(entity_type, entity_id)`; it never stores the content itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityType {
    Post,
    Comment,
    ChatMessage,
    Media,
    /// The account itself (actor-level subject, e.g. for graduated penalties).
    Account,
    Profile,
}

impl EntityType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Post => "post",
            Self::Comment => "comment",
            Self::ChatMessage => "chat_message",
            Self::Media => "media",
            Self::Account => "account",
            Self::Profile => "profile",
        }
    }
}

impl fmt::Display for EntityType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl TryFrom<&str> for EntityType {
    type Error = ModerationError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "post" => Ok(Self::Post),
            "comment" => Ok(Self::Comment),
            "chat_message" => Ok(Self::ChatMessage),
            "media" => Ok(Self::Media),
            "account" => Ok(Self::Account),
            "profile" => Ok(Self::Profile),
            other => Err(ModerationError::DomainViolation {
                field: "entity_type".into(),
                message: format!("unknown entity type: '{other}'"),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn string_round_trip() {
        for e in [
            EntityType::Post,
            EntityType::Comment,
            EntityType::ChatMessage,
            EntityType::Media,
            EntityType::Account,
            EntityType::Profile,
        ] {
            assert_eq!(EntityType::try_from(e.as_str()).unwrap(), e);
        }
        assert!(EntityType::try_from("bogus").is_err());
    }
}
