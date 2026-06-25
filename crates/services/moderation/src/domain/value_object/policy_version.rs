use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::ModerationError;

/// The identifier of the policy ruleset a decision was made under (e.g.
/// `"2026.06.1"`). Moderation does not store policy *text* — it pins the
/// *version*, which is what makes a decision auditable and a rollout reversible:
/// every [`Decision`](crate::domain::aggregate::Decision) records the version it
/// was decided under.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PolicyVersion(String);

impl PolicyVersion {
    /// Constructs a policy version, rejecting an empty/blank identifier.
    pub fn new(value: impl Into<String>) -> Result<Self, ModerationError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(ModerationError::DomainViolation {
                field: "policy_version".into(),
                message: "policy version must not be empty".into(),
            });
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PolicyVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_blank() {
        assert!(PolicyVersion::new("").is_err());
        assert!(PolicyVersion::new("   ").is_err());
        assert_eq!(PolicyVersion::new("2026.06.1").unwrap().as_str(), "2026.06.1");
    }
}
