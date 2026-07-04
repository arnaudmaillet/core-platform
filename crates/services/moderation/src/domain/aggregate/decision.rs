use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::value_object::{
    ActionType, DecisionId, PolicyCategory, PolicyVersion, SubjectRef,
};
use crate::error::ModerationError;

/// Who (or what) made a decision.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DecisionAuthor {
    /// A human reviewer (their id).
    Reviewer(String),
    /// An automated rule (its id).
    Rule(String),
}

impl DecisionAuthor {
    pub fn id(&self) -> &str {
        match self {
            Self::Reviewer(id) | Self::Rule(id) => id,
        }
    }

    pub fn is_automated(&self) -> bool {
        matches!(self, Self::Rule(_))
    }
}

/// An entry in the **decision ledger** — the legal evidence record of an integrity
/// action. It is *append-only*: once recorded it is never mutated. A change of
/// mind (an appeal overturn, a re-review) is expressed as a **new** decision via
/// [`Decision::record_reversal`], which references the one it supersedes through
/// `reverses`. The immutability is enforced structurally: this type exposes no
/// setters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Decision {
    id: DecisionId,
    subject: SubjectRef,
    action: ActionType,
    category: PolicyCategory,
    policy_version: PolicyVersion,
    rationale: String,
    author: DecisionAuthor,
    /// Set when this decision reverses an earlier one.
    reverses: Option<DecisionId>,
    decided_at: DateTime<Utc>,
}

/// Inputs to record a fresh decision.
#[derive(Debug, Clone)]
pub struct DecisionParams {
    pub subject: SubjectRef,
    pub action: ActionType,
    pub category: PolicyCategory,
    pub policy_version: PolicyVersion,
    pub rationale: String,
    pub author: DecisionAuthor,
    pub decided_at: DateTime<Utc>,
}

impl Decision {
    /// Records a new decision. Rejects an empty rationale — the evidence record
    /// must always carry a reason.
    pub fn record(params: DecisionParams) -> Result<Self, ModerationError> {
        Self::build(params, None)
    }

    /// Records a decision that reverses `reverses` (e.g. an appeal overturn). The
    /// original is left untouched; this is a separate ledger entry.
    pub fn record_reversal(
        params: DecisionParams,
        reverses: DecisionId,
    ) -> Result<Self, ModerationError> {
        Self::build(params, Some(reverses))
    }

    fn build(params: DecisionParams, reverses: Option<DecisionId>) -> Result<Self, ModerationError> {
        if params.rationale.trim().is_empty() {
            return Err(ModerationError::DomainViolation {
                field: "decision.rationale".into(),
                message: "a decision must record a rationale".into(),
            });
        }
        Ok(Self {
            id: DecisionId::new(),
            subject: params.subject,
            action: params.action,
            category: params.category,
            policy_version: params.policy_version,
            rationale: params.rationale,
            author: params.author,
            reverses,
            decided_at: params.decided_at,
        })
    }

    /// Reconstructs a decision from storage (no validation).
    #[allow(clippy::too_many_arguments)]
    pub fn reconstitute(
        id: DecisionId,
        subject: SubjectRef,
        action: ActionType,
        category: PolicyCategory,
        policy_version: PolicyVersion,
        rationale: String,
        author: DecisionAuthor,
        reverses: Option<DecisionId>,
        decided_at: DateTime<Utc>,
    ) -> Self {
        Self {
            id,
            subject,
            action,
            category,
            policy_version,
            rationale,
            author,
            reverses,
            decided_at,
        }
    }

    pub fn id(&self) -> DecisionId {
        self.id
    }

    pub fn subject(&self) -> &SubjectRef {
        &self.subject
    }

    pub fn action(&self) -> ActionType {
        self.action
    }

    pub fn category(&self) -> PolicyCategory {
        self.category
    }

    pub fn policy_version(&self) -> &PolicyVersion {
        &self.policy_version
    }

    pub fn rationale(&self) -> &str {
        &self.rationale
    }

    pub fn author(&self) -> &DecisionAuthor {
        &self.author
    }

    pub fn is_automated(&self) -> bool {
        self.author.is_automated()
    }

    pub fn reverses(&self) -> Option<DecisionId> {
        self.reverses
    }

    pub fn decided_at(&self) -> DateTime<Utc> {
        self.decided_at
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::value_object::{ActorId, EntityType};
    use uuid::Uuid;

    fn t0() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-06-25T12:00:00Z").unwrap().with_timezone(&Utc)
    }

    fn params(rationale: &str) -> DecisionParams {
        DecisionParams {
            subject: SubjectRef::new(EntityType::Post, "p1", ActorId::from_uuid(Uuid::from_u128(1)), "feed").unwrap(),
            action: ActionType::RemoveContent,
            category: PolicyCategory::Hate,
            policy_version: PolicyVersion::new("2026.06.1").unwrap(),
            rationale: rationale.into(),
            author: DecisionAuthor::Reviewer("rev-7".into()),
            decided_at: t0(),
        }
    }

    #[test]
    fn record_requires_rationale() {
        assert!(matches!(
            Decision::record(params("  ")).unwrap_err(),
            ModerationError::DomainViolation { .. }
        ));
    }

    #[test]
    fn fresh_decision_has_no_reversal_and_unique_id() {
        let d1 = Decision::record(params("violates hate policy")).unwrap();
        let d2 = Decision::record(params("violates hate policy")).unwrap();
        assert!(d1.reverses().is_none());
        assert_ne!(d1.id(), d2.id());
        assert!(!d1.is_automated());
    }

    #[test]
    fn reversal_references_the_original() {
        let original = Decision::record(params("removed in error")).unwrap();
        let mut p = params("overturned on appeal");
        p.action = ActionType::NoAction;
        let reversal = Decision::record_reversal(p, original.id()).unwrap();
        assert_eq!(reversal.reverses(), Some(original.id()));
    }

    #[test]
    fn automated_author_is_flagged() {
        let mut p = params("auto-removed: known-bad hash");
        p.author = DecisionAuthor::Rule("hash-match-v1".into());
        let d = Decision::record(p).unwrap();
        assert!(d.is_automated());
        assert_eq!(d.author().id(), "hash-match-v1");
    }
}
