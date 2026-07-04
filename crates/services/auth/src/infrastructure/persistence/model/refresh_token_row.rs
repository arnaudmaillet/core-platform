use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::domain::aggregate::RefreshToken;
use crate::domain::value_object::{
    AccountId, RefreshTokenHash, RefreshTokenId, RefreshTokenStatus, SessionId,
};
use crate::error::AuthError;

/// Flat projection of the `refresh_tokens` table.
#[derive(Debug, sqlx::FromRow)]
pub struct RefreshTokenRow {
    pub id: Uuid,
    pub session_id: Uuid,
    pub account_id: Uuid,
    pub token_hash: String,
    pub status: String,
    pub issued_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub used_at: Option<DateTime<Utc>>,
    pub replaced_by: Option<Uuid>,
    pub version: i64,
}

impl TryFrom<RefreshTokenRow> for RefreshToken {
    type Error = AuthError;

    fn try_from(row: RefreshTokenRow) -> Result<Self, Self::Error> {
        let status = RefreshTokenStatus::try_from(row.status.as_str())?;
        let token_hash = RefreshTokenHash::new(row.token_hash)?;

        Ok(RefreshToken::reconstitute(
            RefreshTokenId::from_uuid(row.id),
            SessionId::from_uuid(row.session_id),
            AccountId::from_uuid(row.account_id),
            token_hash,
            status,
            row.issued_at,
            row.expires_at,
            row.used_at,
            row.replaced_by.map(RefreshTokenId::from_uuid),
            row.version,
        ))
    }
}
