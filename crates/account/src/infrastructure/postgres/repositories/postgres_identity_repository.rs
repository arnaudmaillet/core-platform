// crates/account/src/infrastructure/postgres/repositories/account_repository

use std::sync::Arc;
use std::time::Duration;
use crate::domain::account::entities::AccountIdentity;
use crate::domain::repositories::AccountIdentityRepository;
use crate::domain::value_objects::{AccountState, Email, ExternalId, PhoneNumber};
use crate::infrastructure::postgres::rows::PostgresAccountIdentityRow;
use async_trait::async_trait;
use shared_kernel::domain::Identifier;
use shared_kernel::domain::events::AggregateRoot;
use shared_kernel::domain::repositories::{CacheRepository, CacheRepositoryExt};
use shared_kernel::domain::transaction::Transaction;
use shared_kernel::domain::value_objects::AccountId;
use shared_kernel::errors::Result;
use shared_kernel::infrastructure::postgres::mappers::SqlxErrorExt;
use sqlx::{Pool, Postgres, query, query_as, query_scalar};

pub struct PostgresAccountIdentityRepository {
    pool: Pool<Postgres>,
    cache: Arc<dyn CacheRepository>,
}

impl PostgresAccountIdentityRepository {
    pub fn new(pool: Pool<Postgres>, cache: Arc<dyn CacheRepository>) -> Self {
        Self { pool, cache }
    }

    fn cache_key(account_id: &AccountId) -> String {
        format!("account:identity:{}", account_id.as_string())
    }
}

#[async_trait]
impl AccountIdentityRepository for PostgresAccountIdentityRepository {
    // --- RÉSOLUTIONS & LECTURES ---

    async fn fetch_by_account_id(
        &self,
        account_id: &AccountId,
        tx: Option<&mut dyn Transaction>,
    ) -> Result<Option<AccountIdentity>> {
        let key = Self::cache_key(account_id);
        let should_use_cache = tx.is_none();

        // 1. Read-through
        if should_use_cache {
            if let Ok(Some(identity)) = self.cache.get_obj::<AccountIdentity>(&key).await {
                return Ok(Some(identity));
            }
        }

        // 2. Read from DB
        let uid = account_id.as_uuid();
        let row: Option<PostgresAccountIdentityRow> =
            <dyn Transaction>::execute_on(&self.pool, tx, |conn| {
                Box::pin(async move {
                    query_as::<_, PostgresAccountIdentityRow>(
                        "SELECT * FROM account_identity WHERE account_id = $1",
                    )
                    .bind(uid)
                    .fetch_optional(conn)
                    .await
                    .map_domain::<AccountIdentity>()
                })
            })
            .await?;

        let identity = row.map(AccountIdentity::try_from).transpose()?;

        // 3. Write-through
        if should_use_cache {
            if let Some(ref ident) = identity {
                let _ = self.cache.set_obj(&key, ident, Some(Duration::from_secs(900))).await;
            }
        }

        Ok(identity)
    }

    async fn resolve_id_from_external_id(&self, ext_id: &ExternalId) -> Result<Option<AccountId>> {
        let account_id = query_scalar::<_, uuid::Uuid>(
            "SELECT account_id FROM account_identity WHERE external_id = $1",
        )
        .bind(ext_id.as_str())
        .fetch_optional(&self.pool)
        .await
        .map_domain::<AccountIdentity>()?;

        Ok(account_id.map(AccountId::from_uuid))
    }

    async fn resolve_id_from_email(&self, email: &Email) -> Result<Option<AccountId>> {
        let account_id = query_scalar::<_, uuid::Uuid>(
            "SELECT account_id FROM account_identity WHERE email = $1",
        )
        .bind(email.as_str())
        .fetch_optional(&self.pool)
        .await
        .map_domain::<AccountIdentity>()?;

        Ok(account_id.map(AccountId::from_uuid))
    }

    // --- VÉRIFICATIONS ---

    async fn exists_by_external_id(&self, ext_id: &ExternalId) -> Result<bool> {
        query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM account_identity WHERE external_id = $1)",
        )
        .bind(ext_id.as_str())
        .fetch_one(&self.pool)
        .await
        .map_domain::<AccountIdentity>()
    }

    async fn exists_by_email(&self, email: &Email) -> Result<bool> {
        query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM account_identity WHERE email = $1)")
            .bind(email.as_str())
            .fetch_one(&self.pool)
            .await
            .map_domain::<AccountIdentity>()
    }

    async fn exists_by_phone(&self, phone: &PhoneNumber) -> Result<bool> {
        query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM account_identity WHERE phone_number = $1)",
        )
        .bind(phone.as_str())
        .fetch_one(&self.pool)
        .await
        .map_domain::<AccountIdentity>()
    }

    // --- MUTATIONS ---

    async fn save(
        &self,
        account: &AccountIdentity,
        original: Option<&AccountIdentity>,
        tx: Option<&mut dyn Transaction>,
    ) -> Result<()> {
        // 1. Persist sql
        self.execute_upsert(account, original, tx).await?;

        // 2. Invalidate cache
        let _ = self.cache.delete(&Self::cache_key(account.account_id())).await;

        Ok(())
    }

    async fn transit_to_state(
        &self,
        account_id: &AccountId,
        state: AccountState,
        tx: &mut dyn Transaction,
    ) -> Result<()> {
        let uid = account_id.as_uuid();
        let state_str = state.as_str().to_string();

        <dyn Transaction>::execute_on(&self.pool, Some(tx), |conn| {
            Box::pin(async move {
                query("UPDATE account_identity SET state = $1, version = version + 1, updated_at = NOW() WHERE account_id = $2")
                    .bind(state_str)
                    .bind(uid)
                    .execute(conn)
                    .await
                    .map_domain::<AccountIdentity>()
            })
        }).await?;

        let _ = self.cache.delete(&Self::cache_key(account_id)).await;

        Ok(())
    }

    async fn record_activity(&self, account_id: &AccountId) -> Result<()> {
        query("UPDATE account_identity SET last_active_at = NOW() WHERE account_id = $1")
            .bind(account_id.as_uuid())
            .execute(&self.pool)
            .await
            .map_domain::<AccountIdentity>()?;

        let _ = self.cache.delete(&Self::cache_key(account_id)).await;

        Ok(())
    }

    async fn hard_delete(&self, account_id: &AccountId, tx: &mut dyn Transaction) -> Result<()> {
        let uid = account_id.as_uuid();

        <dyn Transaction>::execute_on(&self.pool, Some(tx), |conn| {
            Box::pin(async move {
                query("DELETE FROM account_identity WHERE account_id = $1")
                    .bind(uid)
                    .execute(conn)
                    .await
                    .map_domain::<AccountIdentity>()
            })
        })
        .await?;

        let _ = self.cache.delete(&Self::cache_key(account_id)).await;

        Ok(())
    }
}

impl PostgresAccountIdentityRepository {
    async fn execute_upsert(
        &self,
        identity: &AccountIdentity,
        original: Option<&AccountIdentity>,
        tx: Option<&mut dyn Transaction>,
    ) -> Result<()> {
        // 1. Préparation des données possédées (Owned data)
        let row = PostgresAccountIdentityRow::try_from(identity)?;
        let old_version_i64 = original.map(|o| o.version_i64()).transpose()?.unwrap_or(0);
        let new_version_i64 = row.version;
        
        // On clone l'ID maintenant pour ne pas dépendre de la durée de vie de `identity`
        let account_id_display = identity.account_id().to_string();

        // 2. Exécution via le wrapper de transaction
        // On utilise 'move' sur la clôture pour capturer 'row' et 'account_id_display'
        <dyn Transaction>::execute_on(&self.pool, tx, move |conn| {
            // On clone row ici pour que le Box::pin soit propriétaire des données
            let account_id_display = account_id_display.clone();

            Box::pin(async move {
                let sql = r#"
            INSERT INTO account_identity (
                account_id, region_code, external_id, email, email_verified,
                phone_number, phone_verified, state, birth_date,
                locale, version, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            ON CONFLICT (account_id) DO UPDATE SET
                email = EXCLUDED.email,
                email_verified = EXCLUDED.email_verified,
                phone_number = EXCLUDED.phone_number,
                phone_verified = EXCLUDED.phone_verified,
                state = EXCLUDED.state,
                locale = EXCLUDED.locale,
                version = EXCLUDED.version,
                updated_at = EXCLUDED.updated_at
            WHERE account_identity.version = $13
            "#;

                let result = sqlx::query(sql)
                    .bind(row.account_id)      // $1
                    .bind(row.region_code)     // $2
                    .bind(row.external_id)     // $3
                    .bind(row.email)           // $4
                    .bind(row.email_verified)  // $5
                    .bind(row.phone_number)    // $6
                    .bind(row.phone_verified)  // $7
                    .bind(row.state)           // $8
                    .bind(row.birth_date)      // $9
                    .bind(row.locale)          // $10
                    .bind(new_version_i64)     // $11
                    .bind(row.updated_at)      // $12
                    .bind(old_version_i64)     // $13
                    .execute(conn)
                    .await
                    .map_domain::<AccountIdentity>()?;

                // 3. Gestion du conflit de version (Optimistic Locking)
                if result.rows_affected() == 0 && old_version_i64 > 0 {
                    return Err(shared_kernel::errors::DomainError::ConcurrencyConflict {
                        reason: format!(
                            "Account {}: version mismatch (expected v{})",
                            account_id_display,
                            old_version_i64
                        ),
                    });
                }
                Ok(())
            })
        })
        .await?;

        Ok(())
    }
}