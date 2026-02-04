// crates/account/src/infrastructure/postgres/rows/postgres_account_row

use crate::domain::value_objects::{
    AccountState, BirthDate, Email, ExternalId, Locale, PhoneNumber,
};
use chrono::{DateTime, NaiveDate, Utc};
use shared_kernel::domain::Identifier;
use shared_kernel::domain::events::AggregateRoot;
use shared_kernel::domain::value_objects::{AccountId, RegionCode, Username};
use uuid::Uuid;

use crate::domain::builders::AccountBuilder;
use crate::domain::entities::Account;
use crate::infrastructure::postgres::models::PostgresAccountState;
use shared_kernel::errors::{DomainError, Result};

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
            AccountId::from_uuid(row.id),
            RegionCode::from_raw(row.region_code),
            ExternalId::from_raw(row.external_id),
            Username::from_raw(row.username),
            Email::from_raw(row.email),
            row.email_verified,
            row.phone_number.map(PhoneNumber::from_raw),
            row.phone_verified,
            AccountState::from_raw(row.account_state.into()),
            row.birth_date.map(BirthDate::from_raw),
            Locale::from_raw(row.locale),
            row.version,
            row.created_at,
            row.updated_at,
            row.last_active_at,
        ))
    }
}

impl From<&Account> for PostgresAccountRow {
    fn from(a: &Account) -> Self {
        Self {
            id: a.id().as_uuid(),
            region_code: a.region_code().to_string(),
            external_id: a.external_id().to_string(),
            username: a.username().to_string(),
            email: a.email().to_string(),
            email_verified: a.is_email_verified(),
            phone_number: a.phone_number().as_ref().map(|p| p.to_string()),
            phone_verified: a.is_phone_verified(),
            // On convertit le Value Object AccountState vers le type PostgresAccountState via From/Into
            account_state: PostgresAccountState::from(a.state().clone()),
            birth_date: a.birth_date().as_ref().map(|d| d.value()),
            locale: a.locale().to_string(),
            version: a.version(),
            created_at: a.created_at(),
            updated_at: a.updated_at(),
            last_active_at: a.last_active_at(),
        }
    }
}
