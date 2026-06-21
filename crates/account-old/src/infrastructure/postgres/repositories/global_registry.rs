// crates/account/src/infrastructure/postgres/repositories/postgres_global_registry.rs

use crate::infrastructure::postgres::rows::PostgresGlobalIdentityRow;
use crate::repositories::{GlobalIdentityRegistration, GlobalIdentityRegistry};
use crate::types::{AccountState, RegistrationIdentifier};
use async_trait::async_trait;
use infra_sqlx::sqlx::{self, PgPool};
use shared_kernel::core::{Error, Result};
use shared_kernel::types::AccountId;

const RESERVE_IDENTITY_QUERY: &str = r#"
    INSERT INTO global_identity_registry (account_id, region, sub_id, email_hash, phone_hash, state, created_at, updated_at)
    VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
"#;

const FIND_BY_ACCOUNT_ID_QUERY: &str = r#"
    SELECT * FROM global_identity_registry WHERE account_id = $1
"#;

const FIND_BY_EMAIL_HASH_QUERY: &str = r#"
    SELECT * FROM global_identity_registry WHERE email_hash = $1
"#;

const FIND_BY_PHONE_HASH_QUERY: &str = r#"
    SELECT * FROM global_identity_registry WHERE phone_hash = $1
"#;

const FIND_BY_SUB_ID_QUERY: &str = r#"
    SELECT * FROM global_identity_registry WHERE sub_id = $1
"#;

const UPDATE_IDENTIFIERS_QUERY: &str = r#"
    UPDATE global_identity_registry 
    SET email_hash = $1, phone_hash = $2, updated_at = NOW()
    WHERE account_id = $3
"#;

const UPDATE_STATE_QUERY: &str = r#"
    UPDATE global_identity_registry 
    SET state = $1
    WHERE account_id = $2
"#;

const DELETE_IDENTITY_QUERY: &str = r#"
    DELETE FROM global_identity_registry WHERE account_id = $1
"#;

const PURGE_EXPIRED_RESERVATIONS_QUERY: &str = r#"
    DELETE FROM global_identity_registry 
    WHERE state = 'PENDING' 
      AND created_at < $1
"#;

pub struct PostgresGlobalIdentityRegistry {
    global_pool: PgPool,
}

impl PostgresGlobalIdentityRegistry {
    pub fn new(global_pool: PgPool) -> Self {
        Self { global_pool }
    }
}

#[async_trait]
impl GlobalIdentityRegistry for PostgresGlobalIdentityRegistry {
    async fn reserve(&self, registration: &GlobalIdentityRegistration) -> Result<()> {
        let row = PostgresGlobalIdentityRow::from_domain(registration);

        sqlx::query(RESERVE_IDENTITY_QUERY)
            .bind(row.account_id)
            .bind(&row.region)
            .bind(&row.sub_id)
            .bind(&row.email_hash)
            .bind(&row.phone_hash)
            .bind(&row.state)
            .bind(row.created_at)
            .bind(row.updated_at)
            .execute(&self.global_pool)
            .await
            .map_err(|e| {
                if let Some(db_err) = e.as_database_error() {
                    if db_err.code() == Some(std::borrow::Cow::Borrowed("23505")) {
                        let msg = db_err.message();
                        if msg.contains("uq_global_email") {
                            return Error::validation("email", "This email address is already registered globally.");
                        }
                        if msg.contains("uq_global_phone") {
                            return Error::validation("phone", "This phone is already registered globally.");
                        }
                        if msg.contains("uq_global_sub_id") {
                            return Error::validation("sub_id", "This external identity provider sub_id is already linked to an account.");
                        }
                    }
                }
                Error::database(format!("Global identity reservation failed: {}", e))
            })?;

        Ok(())
    }

    async fn find_by_account_id(
        &self,
        account_id: AccountId,
    ) -> Result<Option<GlobalIdentityRegistration>> {
        let row_opt = sqlx::query_as::<_, PostgresGlobalIdentityRow>(FIND_BY_ACCOUNT_ID_QUERY)
            .bind(account_id.uuid())
            .fetch_optional(&self.global_pool)
            .await
            .map_err(|e| Error::database(format!("Global lookup by account_id failed: {}", e)))?;

        row_opt.map(|row| row.to_domain()).transpose()
    }

    async fn find_by_email_hash(
        &self,
        email_hash: &[u8],
    ) -> Result<Option<GlobalIdentityRegistration>> {
        let row_opt = sqlx::query_as::<_, PostgresGlobalIdentityRow>(FIND_BY_EMAIL_HASH_QUERY)
            .bind(email_hash)
            .fetch_optional(&self.global_pool)
            .await
            .map_err(|e| Error::database(format!("Global lookup by email_hash failed: {}", e)))?;

        row_opt.map(|row| row.to_domain()).transpose()
    }

    async fn find_by_phone_hash(
        &self,
        phone_hash: &[u8],
    ) -> Result<Option<GlobalIdentityRegistration>> {
        let row_opt = sqlx::query_as::<_, PostgresGlobalIdentityRow>(FIND_BY_PHONE_HASH_QUERY)
            .bind(phone_hash)
            .fetch_optional(&self.global_pool)
            .await
            .map_err(|e| Error::database(format!("Global lookup by phone_hash failed: {}", e)))?;

        row_opt.map(|row| row.to_domain()).transpose()
    }

    async fn find_by_sub_id(&self, sub_id: &str) -> Result<Option<GlobalIdentityRegistration>> {
        let row_opt = sqlx::query_as::<_, PostgresGlobalIdentityRow>(FIND_BY_SUB_ID_QUERY)
            .bind(sub_id)
            .fetch_optional(&self.global_pool)
            .await
            .map_err(|e| Error::database(format!("Global lookup by sub_id failed: {}", e)))?;

        row_opt.map(|row| row.to_domain()).transpose()
    }

    async fn update_identifiers(
        &self,
        account_id: AccountId,
        new_identifiers: RegistrationIdentifier,
    ) -> Result<()> {
        let email_hash = new_identifiers.email_hash();
        let phone_hash = new_identifiers.phone_hash();

        sqlx::query(UPDATE_IDENTIFIERS_QUERY)
            .bind(email_hash)
            .bind(phone_hash)
            .bind(account_id.uuid())
            .execute(&self.global_pool)
            .await
            .map_err(|e| {
                if let Some(db_err) = e.as_database_error() {
                    if db_err.code() == Some(std::borrow::Cow::Borrowed("23505")) {
                        let msg = db_err.message();
                        if msg.contains("uq_global_email") {
                            return Error::validation(
                                "email",
                                "This email address is already claimed by another account.",
                            );
                        }
                        if msg.contains("uq_global_phone") {
                            return Error::validation(
                                "phone",
                                "This phone is already claimed by another account.",
                            );
                        }
                    }
                }
                Error::database(format!("Global identity identifier update failed: {}", e))
            })?;

        Ok(())
    }

    async fn update_state(&self, account_id: AccountId, new_state: AccountState) -> Result<()> {
        sqlx::query(UPDATE_STATE_QUERY)
            .bind(new_state.as_str())
            .bind(account_id.uuid())
            .execute(&self.global_pool)
            .await
            .map_err(|e| Error::database(format!("Global state update failed: {}", e)))?;

        Ok(())
    }

    async fn delete(&self, account_id: AccountId) -> Result<()> {
        sqlx::query(DELETE_IDENTITY_QUERY)
            .bind(account_id.uuid())
            .execute(&self.global_pool)
            .await
            .map_err(|e| {
                Error::database(format!("Global identity record deletion failed: {}", e))
            })?;

        Ok(())
    }

    async fn purge_expired_reservations(
        &self,
        expired_before: chrono::DateTime<chrono::Utc>,
    ) -> Result<u64> {
        let result = sqlx::query(PURGE_EXPIRED_RESERVATIONS_QUERY)
            .bind(expired_before)
            .execute(&self.global_pool)
            .await
            .map_err(|e| {
                Error::database(format!(
                    "Global identity registry janitor purge failed: {}",
                    e
                ))
            })?;

        Ok(result.rows_affected())
    }
}
