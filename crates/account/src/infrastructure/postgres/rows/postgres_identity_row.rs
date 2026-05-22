// crates/account/src/infrastructure/postgres/rows/postgres_account_row

use crate::{entities::Account, infrastructure::postgres::models::PostgresAccountState};
use chrono::{DateTime, Utc};
use infra_sqlx::sqlx;
use shared_kernel::core::Identifier;
use uuid::Uuid;

#[derive(Debug, sqlx::FromRow)]
pub struct PostgresAccountIdentityRow {
    pub account_id: Uuid,
    pub sub_id: Option<String>,
    pub region: String,
    pub email: Option<String>,
    pub state: PostgresAccountState,
    pub locale: String,
    pub last_active_at: Option<DateTime<Utc>>,
    // pub phone_number: Option<String>,
    // pub birth_date: Option<NaiveDate>,
    // pub created_at: DateTime<Utc>,
    // #[sqlx(rename = "identity_updated_at")]
    // pub updated_at: DateTime<Utc>,
    // pub aggregate_updated_at: DateTime<Utc>,
    // pub version: i64,
}

impl PostgresAccountIdentityRow {
    pub fn from_domain(account: &Account) -> Self {
        let ident = account.identity();
        Self {
            account_id: ident.account_id().as_uuid(),
            sub_id: ident.sub_id().as_ref().map(|id| id.to_string()),
            region: ident.region().to_string(),
            email: ident.email().as_ref().map(|e| e.to_string()),
            state: ident.state().into(),
            locale: ident.locale().to_string(),
            last_active_at: ident.last_active_at(),
            // phone_number: ident.phone_number().as_ref().map(|p| p.to_string()),
            // birth_date: ident.birth_date().map(|d| d.into()),
            // created_at: ident.created_at(),
            // updated_at: account.updated_at(),
            // aggregate_updated_at: account.updated_at(),
            // version: account.metadata().version() as i64,
        }
    }
}
