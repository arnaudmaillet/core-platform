// crates/account/src/infrastructure/persistence/postgres/account_settings_row.rs
use crate::entities::Account;
use infra_sqlx::sqlx;
#[derive(Debug, sqlx::FromRow)]
pub struct PostgresAccountSettingsRow {
    pub preferences: serde_json::Value,
    pub timezone: String,
    pub push_tokens: Vec<String>,
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
        }
    }
}
