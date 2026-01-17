// crates/account/src/infrastructure/postgres/rows/postgres_account_row

use chrono::{DateTime, NaiveDate, Utc};
use uuid::Uuid;
use shared_kernel::domain::value_objects::{RegionCode, AccountId, Username};
use crate::domain::value_objects::{Email, PhoneNumber, BirthDate, ExternalId, Locale, AccountState};

use shared_kernel::errors::{Result, DomainError};
use crate::domain::entities::Account;
use crate::domain::builders::AccountBuilder;
use crate::infrastructure::postgres::models::PostgresAccountState;

#[derive(Debug, sqlx::FromRow)]
pub struct PostgresAccountRow {
    pub id: Uuid,
    pub region_code: String,
    pub external_id: String,
    pub username: String,
    pub email: String,
    pub email_verified: bool,
    pub phone_number: Option<String>,
    pub phone_verified: bool,
    pub account_state: PostgresAccountState,
    pub birth_date: Option<NaiveDate>,
    pub locale: String,
    pub version: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_active_at: Option<DateTime<Utc>>,
}

impl TryFrom<PostgresAccountRow> for Account {
    type Error = DomainError;

    fn try_from(row: PostgresAccountRow) -> Result<Self> {
        Ok(AccountBuilder::restore(
            AccountId::new_unchecked(row.id),
            RegionCode::new_unchecked(row.region_code),
            ExternalId::new_unchecked(row.external_id),
            Username::new_unchecked(row.username),
            Email::new_unchecked(row.email),
            row.email_verified,
            row.phone_number.map(PhoneNumber::new_unchecked),
            row.phone_verified,
            AccountState::new_unchecked(row.account_state.into()),
            row.birth_date.map(BirthDate::new_unchecked),
            Locale::new_unchecked(row.locale),
            row.version,
            row.created_at,
            row.updated_at,
            row.last_active_at,
        ))
    }
}