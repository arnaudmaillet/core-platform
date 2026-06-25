use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::domain::aggregate::EnforcementAction;
use crate::domain::value_object::{
    ActionType, DecisionId, EnforcementId, EnforcementStatus, EnforcementVersion,
};
use crate::error::ModerationError;

use super::subject_from;

/// Flat projection of the `enforcements` table.
#[derive(Debug, sqlx::FromRow)]
pub struct EnforcementRow {
    pub id: Uuid,
    pub entity_type: String,
    pub entity_id: String,
    pub actor_id: Uuid,
    pub surface: String,
    pub action: String,
    pub status: String,
    pub version: i64,
    pub decision_id: Uuid,
    pub applied_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub reversed_at: Option<DateTime<Utc>>,
}

impl TryFrom<EnforcementRow> for EnforcementAction {
    type Error = ModerationError;

    fn try_from(row: EnforcementRow) -> Result<Self, Self::Error> {
        let subject = subject_from(&row.entity_type, row.entity_id, row.actor_id, row.surface)?;
        Ok(EnforcementAction::reconstitute(
            EnforcementId::from_uuid(row.id),
            subject,
            ActionType::try_from(row.action.as_str())?,
            EnforcementStatus::try_from(row.status.as_str())?,
            EnforcementVersion::from_i64(row.version),
            DecisionId::from_uuid(row.decision_id),
            row.applied_at,
            row.expires_at,
            row.reversed_at,
        ))
    }
}
