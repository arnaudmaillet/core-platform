use chrono::{DateTime, Utc};
use sqlx::types::Json;
use uuid::Uuid;

use crate::domain::aggregate::Case;
use crate::domain::value_object::{CaseId, CaseStatus, PolicyCategory, Signal};
use crate::error::ModerationError;

use super::subject_from;

/// Flat projection of the `cases` table.
#[derive(Debug, sqlx::FromRow)]
pub struct CaseRow {
    pub id: Uuid,
    pub entity_type: String,
    pub entity_id: String,
    pub actor_id: Uuid,
    pub surface: String,
    pub status: String,
    pub category: String,
    pub queue: String,
    pub priority: String,
    pub assignee: Option<String>,
    pub signals: Json<Vec<Signal>>,
    pub opened_at: DateTime<Utc>,
    pub version: i64,
}

impl TryFrom<CaseRow> for Case {
    type Error = ModerationError;

    fn try_from(row: CaseRow) -> Result<Self, Self::Error> {
        let subject = subject_from(&row.entity_type, row.entity_id, row.actor_id, row.surface)?;
        Ok(Case::reconstitute(
            CaseId::from_uuid(row.id),
            subject,
            CaseStatus::try_from(row.status.as_str())?,
            PolicyCategory::try_from(row.category.as_str())?,
            row.queue,
            row.priority,
            row.assignee,
            row.signals.0,
            row.opened_at,
            row.version,
        ))
    }
}
