// crates/account/src/infrastructure/persistence/postgres/account_settings_row.rs

use shared_kernel::{
    domain::{
        Identifier,
        value_objects::{AccountId, PushToken, Timezone},
    },
    errors::{DomainError, Result},
};
use uuid::Uuid;

use crate::domain::account::entities::{AccountPreferences, AccountSettings};

#[derive(Debug, sqlx::FromRow)]
pub struct PostgresAccountSettingsRow {
    pub account_id: Uuid,
    pub preferences: serde_json::Value,
    pub timezone: String,
    pub push_tokens: Vec<String>,
}

impl PostgresAccountSettingsRow {
    /// Mappe la ligne SQL vers l'entité de domaine Settings.
    pub fn to_domain(self) -> Result<AccountSettings> {
        let preferences: AccountPreferences = serde_json::from_value(self.preferences)
            .map_err(|e| DomainError::Internal(format!("Désérialisation JSON échouée: {}", e)))?;

        let push_tokens = self
            .push_tokens
            .into_iter()
            .map(PushToken::try_new)
            .collect::<Result<Vec<_>>>()?;

        Ok(AccountSettings::restore(
            AccountId::from_uuid(self.account_id),
            preferences,
            Timezone::try_new(&self.timezone)?,
            push_tokens,
        ))
    }

    pub fn from_domain(account: &crate::domain::account::entities::Account) -> Self {
        let settings = account.settings();

        let preferences =
            serde_json::to_value(settings.preferences()).unwrap_or(serde_json::Value::Null);

        let push_tokens = settings
            .push_tokens()
            .iter()
            .map(|token| token.to_string())
            .collect();

        Self {
            account_id: account.identity().account_id().as_uuid(),
            preferences,
            timezone: settings.timezone().as_str().to_string(),
            push_tokens,
        }
    }
}
