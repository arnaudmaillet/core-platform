use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::domain::aggregate::SubjectLink;
use crate::domain::value_object::{AccountId, IdpSubject};
use crate::error::AuthError;

/// Flat projection of the `subject_links` table.
#[derive(Debug, sqlx::FromRow)]
pub struct SubjectLinkRow {
    pub issuer: String,
    pub subject: String,
    pub account_id: Uuid,
    pub linked_at: DateTime<Utc>,
    pub version: i64,
}

impl TryFrom<SubjectLinkRow> for SubjectLink {
    type Error = AuthError;

    fn try_from(row: SubjectLinkRow) -> Result<Self, Self::Error> {
        let subject = IdpSubject::new(row.issuer, row.subject)?;
        Ok(SubjectLink::reconstitute(
            subject,
            AccountId::from_uuid(row.account_id),
            row.linked_at,
            row.version,
        ))
    }
}
