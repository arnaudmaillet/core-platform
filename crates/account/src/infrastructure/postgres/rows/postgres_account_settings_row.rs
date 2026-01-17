// crates/account/src/infrastructure/persistence/postgres/account_settings_row.rs

use serde::{Deserialize, Serialize};
use shared_kernel::domain::value_objects::{PushToken, RegionCode, Timezone, AccountId};
use shared_kernel::errors::{DomainError, Result};
use crate::domain::entities::{AccountSettings, SettingsBlob};
use crate::domain::builders::AccountSettingsBuilder;

#[derive(Debug, sqlx::FromRow)]
pub struct PostgresAccountSettingsRow {
    pub account_id: uuid::Uuid,
    pub region_code: String,
    pub settings: serde_json::Value,
    pub timezone: String,
    pub push_tokens: Vec<String>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub version: i32, // Indispensable pour l'Hyperscale
}

impl TryFrom<PostgresAccountSettingsRow> for AccountSettings {
    type Error = DomainError;

    fn try_from(row: PostgresAccountSettingsRow) -> Result<Self> {
        // 1. Extraire les données du JSONB (Privacy, Notifications, Appearance)
        let blob: SettingsBlob = serde_json::from_value(row.settings)
            .map_err(|e| DomainError::Internal(format!("Désérialisation JSON échouée: {}", e)))?;

        // 2. Transformer les types simples en Value Objects
        let push_tokens = row.push_tokens
            .into_iter()
            .map(PushToken::new_unchecked)
            .collect();

        // 3. Utiliser le Builder Restore pour injecter la version et les métadonnées
        Ok(AccountSettingsBuilder::restore(
            AccountId::new_unchecked(row.account_id),
            RegionCode::new_unchecked(row.region_code),
            blob.privacy,
            blob.notifications,
            blob.appearance,
            Timezone::new_unchecked(row.timezone),
            push_tokens,
            row.updated_at,
            row.version,
        ))
    }
}