// crates/account/src/infrastructure/postgres/rows/identity_row.rs

use crate::{entities::Account, infrastructure::postgres::models::PostgresAccountState};
use chrono::{DateTime, NaiveDate, Utc};
use infra_sqlx::sqlx;
use shared_kernel::core::Identifier;
use uuid::Uuid;

#[derive(Debug, sqlx::FromRow)]
pub struct PostgresAccountIdentityRow {
    pub account_id: Uuid,
    pub sub_id: Option<String>,
    pub email: Option<String>,
    pub email_verified_at: Option<DateTime<Utc>>,
    pub phone: Option<String>,
    pub phone_verified_at: Option<DateTime<Utc>>,
    pub state: PostgresAccountState,
    pub locale: String,
    pub last_active_at: Option<DateTime<Utc>>,
    pub birth_date: Option<NaiveDate>,
}

impl PostgresAccountIdentityRow {
    pub fn from_domain(account: &Account) -> Self {
        let ident = account.identity();
        Self {
            account_id: ident.account_id().as_uuid(),
            sub_id: ident.sub_id().as_ref().map(|id| id.to_string()),
            email: ident.email().as_ref().map(|e| e.to_string()),
            email_verified_at: ident.email_verified_at(),
            phone: ident.phone().as_ref().map(|p| p.to_string()),
            phone_verified_at: ident.phone_verified_at(),
            state: ident.state().into(),
            locale: ident.locale().to_string(),
            last_active_at: ident.last_active_at(),
            birth_date: ident.birth_date().map(|d| d.into()),
        }
    }
}
