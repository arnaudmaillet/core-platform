use serde::{Deserialize, Serialize};

use crate::error::EngagementError;

/// Supported reaction kinds.
///
/// The set of supported kinds is intentionally fixed — adding a new kind requires
/// a code change and migration. However, the weight assigned to each kind is
/// loaded dynamically from [`crate::config::ReactionWeightsConfig`] at startup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReactionKind {
    Heart,
    Fire,
    Rocket,
    Clap,
    Sad,
}

impl ReactionKind {
    /// Returns the lowercase string key used as Redis HASH field name.
    pub fn as_redis_key(self) -> &'static str {
        match self {
            Self::Heart  => "heart",
            Self::Fire   => "fire",
            Self::Rocket => "rocket",
            Self::Clap   => "clap",
            Self::Sad    => "sad",
        }
    }

    /// Parses a Redis HASH field key back into a `ReactionKind`.
    pub fn from_redis_key(s: &str) -> Result<Self, EngagementError> {
        match s {
            "heart"  => Ok(Self::Heart),
            "fire"   => Ok(Self::Fire),
            "rocket" => Ok(Self::Rocket),
            "clap"   => Ok(Self::Clap),
            "sad"    => Ok(Self::Sad),
            other    => Err(EngagementError::UnknownReactionKind { kind: other.to_owned() }),
        }
    }

    /// Returns the ScyllaDB tinyint ordinal (proto value - 1).
    pub fn as_tinyint(self) -> i8 {
        match self {
            Self::Heart  => 1,
            Self::Fire   => 2,
            Self::Rocket => 3,
            Self::Clap   => 4,
            Self::Sad    => 5,
        }
    }

    /// Converts a ScyllaDB tinyint ordinal back to `ReactionKind`.
    pub fn from_tinyint(v: i8) -> Result<Self, EngagementError> {
        match v {
            1 => Ok(Self::Heart),
            2 => Ok(Self::Fire),
            3 => Ok(Self::Rocket),
            4 => Ok(Self::Clap),
            5 => Ok(Self::Sad),
            n => Err(EngagementError::UnknownReactionKind { kind: n.to_string() }),
        }
    }

    /// Converts a proto enum ordinal (1-based, matching the proto ReactionKind enum) to domain type.
    pub fn from_proto(v: i32) -> Result<Self, EngagementError> {
        match v {
            1 => Ok(Self::Heart),
            2 => Ok(Self::Fire),
            3 => Ok(Self::Rocket),
            4 => Ok(Self::Clap),
            5 => Ok(Self::Sad),
            n => Err(EngagementError::UnknownReactionKind { kind: n.to_string() }),
        }
    }

    /// All reaction kinds in a stable order.
    pub fn all() -> &'static [Self] {
        &[Self::Heart, Self::Fire, Self::Rocket, Self::Clap, Self::Sad]
    }
}
