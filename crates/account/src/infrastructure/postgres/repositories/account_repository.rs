// crates/account/src/infrastructure/postgres/repositories/account_repository

use crate::domain::account::entities::Account;
use crate::domain::repositories::AccountRepository;
use crate::domain::value_objects::{AccountState, Email, ExternalId, PhoneNumber};
use crate::infrastructure::postgres::rows::PostgresAccountRow;
use async_trait::async_trait;
use shared_kernel::domain::Identifier;
use shared_kernel::domain::events::AggregateRoot;
use shared_kernel::domain::transaction::Transaction;
use shared_kernel::domain::value_objects::AccountId;
use shared_kernel::errors::Result;
use shared_kernel::infrastructure::postgres::mappers::SqlxErrorExt;
use sqlx::{Pool, Postgres, query, query_as, query_scalar};

pub struct PostgresAccountRepository {
    pool: Pool<Postgres>,
}

impl PostgresAccountRepository {
    pub fn new(pool: Pool<Postgres>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl AccountRepository for PostgresAccountRepository {
    // --- RÉSOLUTIONS & LECTURES ---

    async fn fetch_by_id(
        &self,
        id: &AccountId,
        tx: Option<&mut dyn Transaction>,
    ) -> Result<Option<Account>> {
        let uid = id.as_uuid();
        let row = <dyn Transaction>::execute_on(&self.pool, tx, |conn| {
            Box::pin(async move {
                query_as::<_, PostgresAccountRow>("SELECT * FROM accounts WHERE id = $1")
                    .bind(uid)
                    .fetch_optional(conn)
                    .await
                    .map_domain::<Account>()
            })
        })
        .await?;

        row.map(Account::try_from).transpose()
    }

    async fn resolve_id_from_external_id(&self, ext_id: &ExternalId) -> Result<Option<AccountId>> {
        let id = query_scalar::<_, uuid::Uuid>("SELECT id FROM accounts WHERE external_id = $1")
            .bind(ext_id.as_str())
            .fetch_optional(&self.pool)
            .await
            .map_domain::<Account>()?;

        Ok(id.map(AccountId::from_uuid))
    }

    async fn resolve_id_from_email(&self, email: &Email) -> Result<Option<AccountId>> {
        let id = query_scalar::<_, uuid::Uuid>("SELECT id FROM accounts WHERE email = $1")
            .bind(email.as_str())
            .fetch_optional(&self.pool)
            .await
            .map_domain::<Account>()?;

        Ok(id.map(AccountId::from_uuid))
    }

    // --- VÉRIFICATIONS ---

    async fn exists_by_external_id(&self, ext_id: &ExternalId) -> Result<bool> {
        query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM accounts WHERE external_id = $1)")
            .bind(ext_id.as_str())
            .fetch_one(&self.pool)
            .await
            .map_domain::<Account>()
    }

    async fn exists_by_email(&self, email: &Email) -> Result<bool> {
        query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM accounts WHERE email = $1)")
            .bind(email.as_str())
            .fetch_one(&self.pool)
            .await
            .map_domain::<Account>()
    }

    async fn exists_by_phone(&self, phone: &PhoneNumber) -> Result<bool> {
        query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM accounts WHERE phone_number = $1)")
            .bind(phone.as_str())
            .fetch_one(&self.pool)
            .await
            .map_domain::<Account>()
    }

    // --- MUTATIONS ---

    async fn save(
        &self,
        account: &Account,
        original: Option<&Account>,
        tx: Option<&mut dyn Transaction>,
    ) -> Result<()> {
        let pool = self.pool.clone();

        // Invalidation intelligente si l'email a changé
        if let Some(old) = original {
            if old.email() != account.email() {
                // Ici tu pourrais appeler ton cache : self.cache.delete(...)
            }
        }

        self.execute_upsert(account, tx).await?;

        // Toujours invalider l'entrée principale du cache
        // self.cache.delete(&format!("account:{}", account.id())).await;

        Ok(())
    }

    async fn transit_to_state(
        &self,
        id: &AccountId,
        state: AccountState,
        tx: &mut dyn Transaction,
    ) -> Result<()> {
        let uid = id.as_uuid();
        let state_str = state.as_str().to_string();

        <dyn Transaction>::execute_on(&self.pool, Some(tx), |conn| {
            Box::pin(async move {
                query("UPDATE accounts SET state = $1, version = version + 1, updated_at = NOW() WHERE id = $2")
                    .bind(state_str)
                    .bind(uid)
                    .execute(conn)
                    .await
                    .map_domain::<Account>()
            })
        }).await?;

        Ok(())
    }

    async fn record_activity(&self, id: &AccountId) -> Result<()> {
        query("UPDATE accounts SET last_active_at = NOW() WHERE id = $1")
            .bind(id.as_uuid())
            .execute(&self.pool)
            .await
            .map_domain::<Account>()?;
        Ok(())
    }

    async fn hard_delete(&self, id: &AccountId, tx: &mut dyn Transaction) -> Result<()> {
        let uid = id.as_uuid();

        <dyn Transaction>::execute_on(&self.pool, Some(tx), |conn| {
            Box::pin(async move {
                query("DELETE FROM accounts WHERE id = $1")
                    .bind(uid)
                    .execute(conn)
                    .await
                    .map_domain::<Account>()
            })
        })
        .await?;

        Ok(())
    }
}

impl PostgresAccountRepository {
    async fn execute_upsert(
        &self,
        account: &Account,
        tx: Option<&mut dyn Transaction>,
    ) -> Result<()> {
        let row = PostgresAccountRow::try_from(account)?;
        let account_id_for_err = account.id().to_string();
        let current_version = account.version();

        // On utilise la version du row (qui est déjà en i64/BIGINT)
        let new_version_i64 = row.version;
        let old_version_i64: i64 = if current_version > 1 {
            (current_version - 1) as i64
        } else {
            0
        };

        <dyn Transaction>::execute_on(&self.pool, tx, |conn| {
            Box::pin(async move {
                let sql = r#"
                INSERT INTO accounts (
                    id, region_code, external_id, email, email_verified,
                    phone_number, phone_verified, state, birth_date,
                    locale, version, updated_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
                ON CONFLICT (id) DO UPDATE SET
                    email = EXCLUDED.email,
                    email_verified = EXCLUDED.email_verified,
                    phone_number = EXCLUDED.phone_number,
                    phone_verified = EXCLUDED.phone_verified,
                    state = EXCLUDED.state,
                    locale = EXCLUDED.locale,
                    version = EXCLUDED.version,
                    updated_at = EXCLUDED.updated_at
                WHERE accounts.version = $13
                "#;

                let result = sqlx::query(sql)
                    .bind(row.id) // $1
                    .bind(&row.region_code) // $2
                    .bind(&row.external_id) // $3
                    .bind(&row.email) // $4
                    .bind(row.email_verified) // $5
                    .bind(&row.phone_number) // $6
                    .bind(row.phone_verified) // $7
                    .bind(row.state) // $8
                    .bind(row.birth_date) // $9
                    .bind(&row.locale) // $10
                    .bind(new_version_i64) // $11
                    .bind(row.updated_at) // $12
                    .bind(old_version_i64) // $13
                    .execute(conn)
                    .await
                    .map_domain::<Account>()?;

                if result.rows_affected() == 0 && current_version > 1 {
                    return Err(shared_kernel::errors::DomainError::ConcurrencyConflict {
                        reason: format!("Account {}: version mismatch", account_id_for_err),
                    });
                }
                Ok(())
            })
        })
        .await?;

        Ok(())
    }
}
