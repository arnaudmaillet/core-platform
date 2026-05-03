// crates/account/src/infrastructure/postgres/rows/postgres_account_row.rs

use crate::domain::{
    account::entities::{
        Account, AccountGovernance, AccountIdentity, AccountPreferences, AccountSettings,
    },
    value_objects::{AccountRole, AccountState, BirthDate, IpAddr, Locale, TrustScore},
};
use crate::infrastructure::postgres::models::{PostgresAccountRole, PostgresAccountState};
use shared_kernel::{
    domain::{
        Identifier,
        events::AggregateMetadata,
        value_objects::{AccountId, Email, PhoneNumber, RegionCode, SubId},
    },
    errors::{DomainError, Result},
};

#[derive(Debug, sqlx::FromRow)]
pub struct PostgresAccountRow {
    // --- Identity ---
    pub account_id: uuid::Uuid,
    pub sub_id: Option<String>,
    pub email: Option<String>,
    pub phone_number: Option<String>,
    pub state: PostgresAccountState,
    pub birth_date: Option<chrono::NaiveDate>,
    pub locale: String,
    pub region_code: String,
    pub version: i64,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub identity_updated_at: chrono::DateTime<chrono::Utc>,
    pub aggregate_updated_at: chrono::DateTime<chrono::Utc>,
    pub last_active_at: Option<chrono::DateTime<chrono::Utc>>,

    // --- Governance (Passage en Option pour gérer le LEFT JOIN vide) ---
    pub role: Option<PostgresAccountRole>,
    pub is_beta_tester: Option<bool>,
    pub is_shadowbanned: Option<bool>,
    pub trust_score: Option<i32>,
    pub moderation_notes: Option<String>,
    pub last_moderation_at: Option<chrono::DateTime<chrono::Utc>>,
    pub last_ip_addr: Option<std::net::IpAddr>,
    pub governance_updated_at: Option<chrono::DateTime<chrono::Utc>>,

    // --- Settings (Passage en Option) ---
    pub preferences: Option<serde_json::Value>,
    pub timezone: Option<String>,
    pub push_tokens: Option<Vec<String>>,
    pub settings_updated_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl PostgresAccountRow {
    pub fn to_domain(self) -> Result<Account> {
        let account_id = AccountId::from_uuid(self.account_id);

        // 1. Reconstruction de Identity
        let identity = AccountIdentity::restore(
            account_id,
            RegionCode::try_new(self.region_code)?,
            self.sub_id.map(SubId::try_new).transpose()?,
            self.email.map(Email::try_new).transpose()?,
            self.phone_number.map(PhoneNumber::try_new).transpose()?,
            AccountState::from(self.state),
            self.birth_date.map(BirthDate::from_raw),
            Locale::try_new(self.locale)?,
            self.created_at,
            self.identity_updated_at,
            self.aggregate_updated_at,
            self.last_active_at,
        );

        // 2. Reconstruction de Governance avec fallback
        let governance = if let Some(role) = self.role {
            AccountGovernance::restore(
                account_id,
                AccountRole::from(role),
                self.is_beta_tester.unwrap_or(false),
                self.is_shadowbanned.unwrap_or(false),
                TrustScore::try_new(self.trust_score.unwrap_or(0))?,
                self.last_moderation_at,
                self.moderation_notes,
                self.last_ip_addr.map(IpAddr::from_raw),
                self.governance_updated_at
                    .unwrap_or(self.aggregate_updated_at),
            )
        } else {
            // Audit Résilience : Si la table governance est vide, on crée un défaut
            AccountGovernance::builder(account_id).build()?
        };

        // 3. Reconstruction de Settings avec fallback
        let settings = if let Some(prefs_val) = self.preferences {
            let preferences: AccountPreferences = serde_json::from_value(prefs_val)
                .map_err(|e| DomainError::Internal(format!("JSON settings error: {}", e)))?;

            let tokens = self
                .push_tokens
                .unwrap_or_default()
                .into_iter()
                .map(shared_kernel::domain::value_objects::PushToken::try_new)
                .collect::<Result<Vec<_>>>()?;

            AccountSettings::restore(
                account_id,
                preferences,
                shared_kernel::domain::value_objects::Timezone::try_new(
                    &self.timezone.unwrap_or_else(|| "UTC".to_string()),
                )?,
                tokens,
                self.settings_updated_at
                    .unwrap_or(self.aggregate_updated_at),
            )
        } else {
            // Audit Résilience : Fallback sur les paramètres par défaut
            AccountSettings::builder(account_id).build()?
        };

        // 4. Reconstruction finale
        Ok(Account::restore(
            identity,
            governance,
            settings,
            AggregateMetadata::restore(self.version as u64, self.aggregate_updated_at),
        ))
    }
}
