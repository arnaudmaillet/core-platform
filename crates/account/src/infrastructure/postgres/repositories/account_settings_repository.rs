// crates/account/src/infrastructure/postgres/repositories/account_settings_repository.rs

use crate::domain::entities::{AccountSettings, SettingsBlob};
use crate::domain::repositories::AccountSettingsRepository;
use crate::infrastructure::postgres::rows::PostgresAccountSettingsRow;
use async_trait::async_trait;
use shared_kernel::domain::Identifier;
use shared_kernel::domain::transaction::Transaction;
use shared_kernel::domain::value_objects::{AccountId, PushToken, Timezone};
use shared_kernel::errors::{DomainError, Result};
use shared_kernel::infrastructure::postgres::mappers::SqlxErrorExt;
use sqlx::PgPool;
use shared_kernel::domain::events::AggregateRoot;

pub struct PostgresAccountSettingsRepository {
    pool: PgPool,
}

impl PostgresAccountSettingsRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl AccountSettingsRepository for PostgresAccountSettingsRepository {
    async fn find_by_account_id(
        &self,
        account_id: &AccountId,
        tx: Option<&mut dyn Transaction>,
    ) -> Result<Option<AccountSettings>> {
        let uid = account_id.as_uuid();

        let row = <dyn Transaction>::execute_on(&self.pool, tx, |conn| {
            Box::pin(async move {
                let query = "SELECT account_id, region_code, settings, timezone, push_tokens, version, updated_at
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

        row.map(AccountSettings::try_from).transpose()
    }

    async fn save(
        &self,
        settings: &AccountSettings,
        tx: Option<&mut dyn Transaction>,
    ) -> Result<()> {
        let blob = SettingsBlob {
            privacy: settings.privacy().clone(),
            notifications: settings.notifications().clone(),
            appearance: settings.appearance().clone(),
        };

        let settings_json =
            serde_json::to_value(blob).map_err(|e| DomainError::Internal(e.to_string()))?;

        let push_tokens: Vec<String> = settings
            .push_tokens()
            .iter()
            .map(|t: &PushToken| t.as_str().to_string())
            .collect();

        let uid = settings.account_id().as_uuid();
        let region = settings.region_code().to_string();
        let tz = settings.timezone().to_string();
        let updated_at = settings.updated_at();

        let new_version_i64 = settings.version_i64()?;
        let old_version_i64: i64 = if settings.version() > 1 {
            (settings.version() - 1).try_into()
                .map_err(|_| DomainError::Internal("Version overflow".into()))?
        } else {
            0
        };

        <dyn Transaction>::execute_on(&self.pool, tx, |conn| Box::pin(async move {
            let query = "
                INSERT INTO account_settings (account_id, region_code, settings, timezone, push_tokens, version, updated_at)
                VALUES ($1, $2, $3, $4, $5, $6, $7)
                ON CONFLICT (account_id, region_code) DO UPDATE SET
                    settings = EXCLUDED.settings,
                    timezone = EXCLUDED.timezone,
                    push_tokens = EXCLUDED.push_tokens,
                    version = EXCLUDED.version,
                    updated_at = EXCLUDED.updated_at
                WHERE account_settings.version = $8";

            let result = sqlx::query(query)
                .bind(uid)
                .bind(region)
                .bind(settings_json)
                .bind(tz)
                .bind(push_tokens)
                .bind(new_version_i64)
                .bind(updated_at)
                .bind(old_version_i64)
                .execute(conn)
                .await
                .map_domain_infra("AccountSettings: save")?;

            if result.rows_affected() == 0 && new_version_i64 > 1 {
                return Err(DomainError::ConcurrencyConflict {
                    reason: format!("Concurrency conflict for account {}: version mismatch", uid)
                });
            }

            Ok(())
        }))
            .await
    }

    async fn update_timezone(
        &self,
        account_id: &AccountId,
        timezone: &Timezone,
        tx: Option<&mut dyn Transaction>,
    ) -> Result<()> {
        let uid = account_id.as_uuid();
        let tz = timezone.as_str().to_string();

        <dyn Transaction>::execute_on(&self.pool, tx, |conn| Box::pin(async move {
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
        }))
            .await
    }

    async fn add_push_token(
        &self,
        account_id: &AccountId,
        token: &PushToken,
        tx: Option<&mut dyn Transaction>,
    ) -> Result<()> {
        let uid = account_id.as_uuid();
        let token_str = token.as_str().to_string();

        <dyn Transaction>::execute_on(&self.pool, tx, |conn| Box::pin(async move {
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
        }))
            .await
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
        .await
    }

    async fn delete_for_user(
        &self,
        account_id: &AccountId,
        tx: Option<&mut dyn Transaction>,
    ) -> Result<()> {
        let uid = account_id.as_uuid();

        <dyn Transaction>::execute_on(&self.pool, tx, |conn| {
            Box::pin(async move {
                let query = "DELETE FROM account_settings WHERE account_id = $1";
                sqlx::query(query)
                    .bind(uid)
                    .execute(conn)
                    .await
                    .map_domain_infra("AccountSettings: delete_user")?;
                Ok(())
            })
        })
        .await
    }
}
