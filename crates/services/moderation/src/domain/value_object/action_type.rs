use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::ModerationError;

/// The executable consequence of a decision, ordered least-to-most severe.
///
/// Moderation **decides** the action; the owning service **executes** it (content
/// services flip visibility; `account` runs suspension/ban lifecycle). The
/// [`ActionType::severity_rank`] ordering is what the graduated-enforcement engine
/// uses to escalate — never a hard-coded threshold.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionType {
    /// No enforcement — the case is dismissed as non-violating.
    NoAction,
    Warn,
    /// Shadow-reduce: content stays but its reach/visibility is limited.
    VisibilityLimit,
    AgeGate,
    RemoveContent,
    /// Actor-level: restrict the actor's ability to post/interact for a window.
    RestrictActor,
    Suspend,
    Ban,
}

impl ActionType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::NoAction => "no_action",
            Self::Warn => "warn",
            Self::VisibilityLimit => "visibility_limit",
            Self::AgeGate => "age_gate",
            Self::RemoveContent => "remove_content",
            Self::RestrictActor => "restrict_actor",
            Self::Suspend => "suspend",
            Self::Ban => "ban",
        }
    }

    /// Monotonic severity used to escalate and to pick the more severe of two
    /// candidate actions. Higher = harsher.
    pub fn severity_rank(&self) -> u8 {
        match self {
            Self::NoAction => 0,
            Self::Warn => 1,
            Self::VisibilityLimit => 2,
            Self::AgeGate => 3,
            Self::RemoveContent => 4,
            Self::RestrictActor => 5,
            Self::Suspend => 6,
            Self::Ban => 7,
        }
    }

    /// Whether the action targets the actor (account) rather than a single piece
    /// of content. Actor-level actions are executed by `account`.
    pub fn is_actor_level(&self) -> bool {
        matches!(self, Self::RestrictActor | Self::Suspend | Self::Ban)
    }

    /// Whether the action results in an [`EnforcementAction`] at all. `NoAction`
    /// is a dismissal — it records a [`Decision`] but creates no enforcement.
    ///
    /// [`EnforcementAction`]: crate::domain::aggregate::EnforcementAction
    /// [`Decision`]: crate::domain::aggregate::Decision
    pub fn is_enforced(&self) -> bool {
        !matches!(self, Self::NoAction)
    }

    /// The harsher of two actions (by severity rank).
    pub fn max(self, other: ActionType) -> ActionType {
        if other.severity_rank() > self.severity_rank() {
            other
        } else {
            self
        }
    }
}

impl fmt::Display for ActionType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl TryFrom<&str> for ActionType {
    type Error = ModerationError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "no_action" => Ok(Self::NoAction),
            "warn" => Ok(Self::Warn),
            "visibility_limit" => Ok(Self::VisibilityLimit),
            "age_gate" => Ok(Self::AgeGate),
            "remove_content" => Ok(Self::RemoveContent),
            "restrict_actor" => Ok(Self::RestrictActor),
            "suspend" => Ok(Self::Suspend),
            "ban" => Ok(Self::Ban),
            other => Err(ModerationError::DomainViolation {
                field: "action_type".into(),
                message: format!("unknown action type: '{other}'"),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn severity_is_strictly_ordered() {
        let ordered = [
            ActionType::NoAction,
            ActionType::Warn,
            ActionType::VisibilityLimit,
            ActionType::AgeGate,
            ActionType::RemoveContent,
            ActionType::RestrictActor,
            ActionType::Suspend,
            ActionType::Ban,
        ];
        for w in ordered.windows(2) {
            assert!(w[0].severity_rank() < w[1].severity_rank());
        }
    }

    #[test]
    fn max_picks_the_harsher() {
        assert_eq!(ActionType::Warn.max(ActionType::Ban), ActionType::Ban);
        assert_eq!(ActionType::Suspend.max(ActionType::Warn), ActionType::Suspend);
        assert_eq!(ActionType::NoAction.max(ActionType::NoAction), ActionType::NoAction);
    }

    #[test]
    fn actor_level_and_enforced_predicates() {
        assert!(ActionType::Ban.is_actor_level());
        assert!(!ActionType::RemoveContent.is_actor_level());
        assert!(!ActionType::NoAction.is_enforced());
        assert!(ActionType::Warn.is_enforced());
    }

    #[test]
    fn string_round_trip() {
        for a in [
            ActionType::NoAction,
            ActionType::Warn,
            ActionType::VisibilityLimit,
            ActionType::AgeGate,
            ActionType::RemoveContent,
            ActionType::RestrictActor,
            ActionType::Suspend,
            ActionType::Ban,
        ] {
            assert_eq!(ActionType::try_from(a.as_str()).unwrap(), a);
        }
        assert!(ActionType::try_from("bogus").is_err());
    }
}
