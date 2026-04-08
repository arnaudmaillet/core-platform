// crates/account/src/infrastructure/persistence/postgres/account_settings_row.rs

use crate::domain::account::builders::AccountSettingsBuilder;
use crate::domain::account::entities::{AccountSettings, AccountPreferences};
use shared_kernel::domain::events::AggregateMetadata;
use shared_kernel::domain::Identifier;
use shared_kernel::domain::value_objects::{AccountId, PushToken, RegionCode, Timezone};
use shared_kernel::errors::{DomainError, Result};

#[derive(Debug, sqlx::FromRow)]
pub struct PostgresAccountSettingsRow {
    pub account_id: uuid::Uuid,
    pub preferences: serde_json::Value,
    pub timezone: String,
    pub push_tokens: Vec<String>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub version: i64,
}

impl TryFrom<PostgresAccountSettingsRow> for AccountSettings {
    type Error = DomainError;

    fn try_from(row: PostgresAccountSettingsRow) -> Result<Self> {
        let preferences: AccountPreferences = serde_json::from_value(row.preferences)
            .map_err(|e| DomainError::Internal(format!("Désérialisation JSON échouée: {}", e)))?;

        let metadata = AggregateMetadata::try_from(row.version)?;

        let push_tokens = row
            .push_tokens
            .into_iter()
            .map(PushToken::from_raw)
            .collect();

        Ok(AccountSettingsBuilder::restore(
            AccountId::from_uuid(row.account_id),
            preferences,
            Timezone::from_raw(row.timezone),
            push_tokens,
            row.updated_at,
            metadata.version(),
        ))
    }
}
