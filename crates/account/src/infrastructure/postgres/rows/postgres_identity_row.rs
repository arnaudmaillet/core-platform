// crates/account/src/infrastructure/postgres/rows/postgres_account_row

use chrono::{DateTime, NaiveDate, Utc};
use shared_kernel::{
    domain::{
        Identifier,
        events::AggregateRoot,
        value_objects::{AccountId, RegionCode},
    },
    errors::Result,
};
use uuid::Uuid;

use crate::{
    domain::{
        account::entities::{Account, AccountIdentity},
        value_objects::{AccountState, BirthDate, Email, ExternalId, Locale, PhoneNumber},
    },
    infrastructure::postgres::models::PostgresAccountState,
};

#[derive(Debug, sqlx::FromRow)]
pub struct PostgresAccountIdentityRow {
    pub account_id: Uuid,
    pub region_code: String,
    pub external_id: Option<String>,
    pub email: Option<String>,
    pub email_verified: bool,
    pub phone_number: Option<String>,
    pub phone_verified: bool,
    pub state: PostgresAccountState,
    pub birth_date: Option<NaiveDate>,
    pub locale: String,
    pub version: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub aggregate_updated_at: DateTime<Utc>,
    pub last_active_at: Option<DateTime<Utc>>,
}

impl PostgresAccountIdentityRow {
    pub fn to_domain(self) -> Result<AccountIdentity> {
        Ok(AccountIdentity::restore(
            AccountId::from_uuid(self.account_id),
            RegionCode::try_new(&self.region_code)?,
            self.external_id
                .map(|id| ExternalId::try_new(id))
                .transpose()?,
            self.email.as_deref().map(Email::try_new).transpose()?,
            self.email_verified,
            self.phone_number
                .as_deref()
                .map(PhoneNumber::try_new)
                .transpose()?,
            self.phone_verified,
            AccountState::from(self.state),
            self.birth_date.map(BirthDate::from_raw),
            Locale::try_new(&self.locale)?,
            self.created_at,
            self.last_active_at,
        ))
    }

    pub fn from_domain(account: &Account) -> Self {
        let ident = account.identity();
        Self {
            account_id: ident.account_id().as_uuid(),
            region_code: ident.region_code().to_string(),
            external_id: ident.external_id().as_ref().map(|id| id.to_string()),
            email: ident.email().as_ref().map(|e| e.to_string()),
            email_verified: ident.is_email_verified(),
            phone_number: ident.phone_number().as_ref().map(|p| p.to_string()),
            phone_verified: ident.is_phone_verified(),
            state: ident.state().into(),
            birth_date: ident.birth_date().map(|d| d.into()),
            locale: ident.locale().to_string(),
            version: account.metadata().version() as i64,
            created_at: ident.created_at(),
            updated_at: account.updated_at(),
            aggregate_updated_at: account.updated_at(),
            last_active_at: ident.last_active_at(),
        }
    }
}
