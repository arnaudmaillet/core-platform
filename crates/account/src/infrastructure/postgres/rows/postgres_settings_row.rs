// crates/account/src/infrastructure/persistence/postgres/account_settings_row.rs
use crate::account::entities::Account;

#[derive(Debug, sqlx::FromRow)]
pub struct PostgresAccountSettingsRow {
    pub preferences: serde_json::Value,
    pub timezone: String,
    pub push_tokens: Vec<String>,
    // pub account_id: Uuid,
    // #[sqlx(rename = "settings_updated_at")]
    // pub updated_at: DateTime<Utc>,
}

impl PostgresAccountSettingsRow {
    pub fn from_domain(account: &Account) -> Self {
        let settings = account.settings();

        let preferences =
            serde_json::to_value(settings.preferences()).unwrap_or(serde_json::Value::Null);

        let push_tokens = settings
            .push_tokens()
            .iter()
            .map(|token| token.to_string())
            .collect();

        Self {
            preferences,
            timezone: settings.timezone().as_str().to_string(),
            push_tokens,
            // account_id: account.identity().account_id().as_uuid(),
            // updated_at: settings.updated_at(),
        }
    }
}
