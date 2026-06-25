use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::domain::aggregate::{Decision, DecisionAuthor};
use crate::domain::value_object::{ActionType, DecisionId, PolicyCategory, PolicyVersion};
use crate::error::ModerationError;

use super::subject_from;

/// Flat projection of the `decisions` table (append-only ledger).
#[derive(Debug, sqlx::FromRow)]
pub struct DecisionRow {
    pub id: Uuid,
    pub entity_type: String,
    pub entity_id: String,
    pub actor_id: Uuid,
    pub surface: String,
    pub action: String,
    pub category: String,
    pub policy_version: String,
    pub rationale: String,
    pub author_kind: String,
    pub author_id: String,
    pub reverses: Option<Uuid>,
    pub decided_at: DateTime<Utc>,
}

impl TryFrom<DecisionRow> for Decision {
    type Error = ModerationError;

    fn try_from(row: DecisionRow) -> Result<Self, Self::Error> {
        let subject = subject_from(&row.entity_type, row.entity_id, row.actor_id, row.surface)?;
        let author = match row.author_kind.as_str() {
            "reviewer" => DecisionAuthor::Reviewer(row.author_id),
            "rule" => DecisionAuthor::Rule(row.author_id),
            other => {
                return Err(ModerationError::DomainViolation {
                    field: "decision.author_kind".into(),
                    message: format!("unknown author kind: '{other}'"),
                });
            }
        };
        Ok(Decision::reconstitute(
            DecisionId::from_uuid(row.id),
            subject,
            ActionType::try_from(row.action.as_str())?,
            PolicyCategory::try_from(row.category.as_str())?,
            PolicyVersion::new(row.policy_version)?,
            row.rationale,
            author,
            row.reverses.map(DecisionId::from_uuid),
            row.decided_at,
        ))
    }
}
