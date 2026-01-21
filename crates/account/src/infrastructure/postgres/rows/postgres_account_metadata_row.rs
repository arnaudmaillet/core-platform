use sqlx::FromRow;
use uuid::Uuid;
use chrono::{DateTime, Utc};
use shared_kernel::domain::Identifier;
use shared_kernel::domain::value_objects::{RegionCode, AccountId};
use shared_kernel::errors::{DomainError, Result};
use crate::domain::builders::AccountMetadataBuilder;
use crate::domain::entities::AccountMetadata;
use crate::domain::value_objects::AccountRole;
use crate::infrastructure::postgres::models::PostgresAccountRole;

#[derive(Debug, FromRow)]
pub struct PostgresAccountMetadataRow {
    pub account_id: Uuid,
    pub region_code: String,
    pub role: PostgresAccountRole,
    pub is_beta_tester: bool,
    pub is_shadowbanned: bool,
    pub trust_score: i32,
    pub last_moderation_at: Option<DateTime<Utc>>,
    pub moderation_notes: Option<String>,
    pub estimated_ip: Option<String>,
    pub updated_at: DateTime<Utc>,
    pub version: i32,
}

impl TryFrom<PostgresAccountMetadataRow> for AccountMetadata {
    type Error = DomainError;

    fn try_from(row: PostgresAccountMetadataRow) -> Result<Self> {
        Ok(AccountMetadataBuilder::restore(
            AccountId::from_uuid(row.account_id),
            RegionCode::from_raw(row.region_code),
            AccountRole::from_raw(row.role.into()),
            row.is_beta_tester,
            row.is_shadowbanned,
            row.trust_score,
            row.last_moderation_at,
            row.moderation_notes,
            row.estimated_ip,
            row.updated_at,
            row.version,
        ))
    }
}