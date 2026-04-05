use crate::domain::account::entities::AccountMetadata;
use crate::domain::account::builders::AccountMetadataBuilder;
use crate::domain::value_objects::AccountRole;
use std::net::IpAddr as StdIpAddr;
use crate::domain::value_objects::IpAddr;
use crate::infrastructure::postgres::models::PostgresAccountRole;
use chrono::{DateTime, Utc};
use shared_kernel::domain::Identifier;
use shared_kernel::domain::events::AggregateMetadata;
use shared_kernel::domain::value_objects::{AccountId, RegionCode};
use shared_kernel::errors::{DomainError, Result};
use sqlx::FromRow;
use uuid::Uuid;

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
    pub last_ip_addr: Option<StdIpAddr>,
    pub updated_at: DateTime<Utc>,
    pub version: i64,
}

impl TryFrom<PostgresAccountMetadataRow> for AccountMetadata {
    type Error = DomainError;

    fn try_from(row: PostgresAccountMetadataRow) -> Result<Self> {
        let last_ip_addr = row.last_ip_addr
            .map(IpAddr::from_raw);
        let metadata = AggregateMetadata::try_from(row.version)?;

        Ok(AccountMetadataBuilder::restore(
            AccountId::from_uuid(row.account_id),
            RegionCode::from_raw(row.region_code),
            AccountRole::from_raw(row.role.into()),
            row.is_beta_tester,
            row.is_shadowbanned,
            row.trust_score,
            row.last_moderation_at,
            row.moderation_notes,
            last_ip_addr,
            row.updated_at,
            metadata.version(),
        ))
    }
}