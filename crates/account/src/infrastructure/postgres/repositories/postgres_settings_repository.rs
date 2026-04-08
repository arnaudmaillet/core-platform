// crates/account/src/infrastructure/postgres/repositories/account_settings_repository.rs
use crate::domain::account::entities::AccountSettings;
use crate::domain::repositories::AccountSettingsRepository;
use crate::infrastructure::postgres::rows::PostgresAccountSettingsRow;
use async_trait::async_trait;
use shared_kernel::domain::Identifier;
use shared_kernel::domain::events::AggregateRoot;
use shared_kernel::domain::repositories::{CacheRepository, CacheRepositoryExt};
use shared_kernel::domain::transaction::Transaction;
use shared_kernel::domain::value_objects::{AccountId, PushToken, Timezone};
use shared_kernel::errors::{DomainError, Result};
use shared_kernel::infrastructure::postgres::mappers::SqlxErrorExt;
use sqlx::PgPool;
use std::sync::Arc;
use std::time::Duration;

pub struct PostgresAccountSettingsRepository {
    pool: PgPool,
    cache: Arc<dyn CacheRepository>,
}

impl PostgresAccountSettingsRepository {
    pub fn new(pool: PgPool, cache: Arc<dyn CacheRepository>) -> Self {
        Self { pool, cache }
    }

    fn cache_key(account_id: &AccountId) -> String {
        format!("account:settings:{}", account_id.as_string())
    }
}

#[async_trait]
impl AccountSettingsRepository for PostgresAccountSettingsRepository {
    async fn fetch_by_account_id(
        &self,
        account_id: &AccountId,
        tx: Option<&mut dyn Transaction>,
    ) -> Result<Option<AccountSettings>> {
        let key = Self::cache_key(account_id);
        let should_use_cache = tx.is_none();

        // 1. READ-THROUGH
        if should_use_cache {
            if let Ok(Some(settings)) = self.cache.get_obj::<AccountSettings>(&key).await {
                return Ok(Some(settings));
            }
        }

        // 2. READ DB
        let uid = account_id.as_uuid();
        let row = <dyn Transaction>::execute_on(&self.pool, tx, |conn| {
            Box::pin(async move {
                let query =
                    "SELECT account_id, preferences, timezone, push_tokens, version, updated_at
                    FROM account_settings WHERE account_id = $1";

                let res: Option<PostgresAccountSettingsRow> = sqlx::query_as(query)
                    .bind(uid)
                    .fetch_optional(conn)
                    .await
                    .map_domain_infra("AccountSettings: find_by_account_id")?;

                Ok(res)
            })
        })
        .await?;

        let settings = row.map(AccountSettings::try_from).transpose()?;

        // 3. WRITE-THROUGH
        if should_use_cache {
            if let Some(ref s) = settings {
                // TTL de 1 heure (3600s) car les réglages changent peu
                let _ = self
                    .cache
                    .set_obj(&key, s, Some(Duration::from_secs(3600)))
                    .await;
            }
        }

        Ok(settings)
    }

    async fn save(
        &self,
        settings: &AccountSettings,
        original: Option<&AccountSettings>,
        tx: Option<&mut dyn Transaction>,
    ) -> Result<()> {
        let settings_json = serde_json::to_value(settings.preferences())
            .map_err(|e| DomainError::Internal(format!("Serialization failed: {}", e)))?;

        let push_tokens: Vec<String> = settings
            .push_tokens()
            .iter()
            .map(|t: &PushToken| t.as_str().to_string())
            .collect();

        let uid = settings.account_id().as_uuid();
        let tz = settings.timezone().to_string();
        let updated_at = settings.updated_at();

        let new_version_i64 = settings.version_i64()?;
        let old_version_i64 = original.map(|o| o.version_i64()).transpose()?.unwrap_or(0);

        <dyn Transaction>::execute_on(&self.pool, tx, |conn| Box::pin(async move {
            let query = "
                INSERT INTO account_settings (account_id, preferences, timezone, push_tokens, version, updated_at)
                VALUES ($1, $2, $3, $4, $5, $6)
                ON CONFLICT (account_id) DO UPDATE SET
                    preferences = EXCLUDED.preferences,
                    timezone = EXCLUDED.timezone,
                    push_tokens = EXCLUDED.push_tokens,
                    version = EXCLUDED.version,
                    updated_at = EXCLUDED.updated_at
                WHERE account_settings.version = $7";

            let result = sqlx::query(query)
                .bind(uid)
                .bind(settings_json)
                .bind(tz)
                .bind(push_tokens)
                .bind(new_version_i64)
                .bind(updated_at)
                .bind(old_version_i64)
                .execute(conn)
                .await
                .map_domain_infra("AccountSettings: save")?;

            if result.rows_affected() == 0 && old_version_i64 > 0 {
                return Err(DomainError::ConcurrencyConflict {
                    reason: format!("OCC Conflict for settings {}: expected v{}", uid, old_version_i64)
                });
            }

            Ok(())
        }))
            .await?;
        let _ = self
            .cache
            .delete(&Self::cache_key(settings.account_id()))
            .await;
        Ok(())
    }

    async fn update_timezone(
        &self,
        account_id: &AccountId,
        timezone: &Timezone,
        tx: Option<&mut dyn Transaction>,
    ) -> Result<()> {
        let uid = account_id.as_uuid();
        let tz = timezone.as_str().to_string();

        <dyn Transaction>::execute_on(&self.pool, tx, |conn| {
            Box::pin(async move {
                let query = "UPDATE account_settings
             SET timezone = $1, version = version + 1, updated_at = NOW()
             WHERE account_id = $2";
                sqlx::query(query)
                    .bind(tz)
                    .bind(uid)
                    .execute(conn)
                    .await
                    .map_domain_infra("AccountSettings: update_timezone")?;
                Ok(())
            })
        })
        .await?;
        let _ = self.cache.delete(&Self::cache_key(account_id)).await;
        Ok(())
    }

    async fn add_push_token(
        &self,
        account_id: &AccountId,
        token: &PushToken,
        tx: Option<&mut dyn Transaction>,
    ) -> Result<()> {
        let uid = account_id.as_uuid();
        let token_str = token.as_str().to_string();

        <dyn Transaction>::execute_on(&self.pool, tx, |conn| {
            Box::pin(async move {
                let query = "UPDATE account_settings
             SET push_tokens = ARRAY(SELECT DISTINCT unnest(array_append(push_tokens, $1))),
                 version = version + 1,
                 updated_at = NOW()
             WHERE account_id = $2";
                sqlx::query(query)
                    .bind(token_str)
                    .bind(uid)
                    .execute(conn)
                    .await
                    .map_domain_infra("AccountSettings: add_push_token")?;
                Ok(())
            })
        })
        .await?;
        let _ = self.cache.delete(&Self::cache_key(account_id)).await;
        Ok(())
    }

    async fn remove_push_token(
        &self,
        account_id: &AccountId,
        token: &PushToken,
        tx: Option<&mut dyn Transaction>,
    ) -> Result<()> {
        let uid = account_id.as_uuid();
        let token_str = token.as_str().to_string();

        <dyn Transaction>::execute_on(&self.pool, tx, |conn| {
            Box::pin(async move {
                let query = "UPDATE account_settings
                         SET push_tokens = array_remove(push_tokens, $1),
                             version = version + 1,
                             updated_at = NOW()
                         WHERE account_id = $2";
                sqlx::query(query)
                    .bind(token_str)
                    .bind(uid)
                    .execute(conn)
                    .await
                    .map_domain_infra("AccountSettings: remove_push_token")?;
                Ok(())
            })
        })
        .await?;
        let _ = self.cache.delete(&Self::cache_key(account_id)).await;
        Ok(())
    }
}
