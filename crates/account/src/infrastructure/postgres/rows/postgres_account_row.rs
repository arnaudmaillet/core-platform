// crates/account/src/infrastructure/postgres/rows/postgres_account_row.rs

use crate::domain::{
    account::entities::{
        Account, AccountGovernance, AccountIdentity, AccountPreferences, AccountSettings,
    },
    value_objects::{
        AccountRole, AccountState, BirthDate, Email, ExternalId, IpAddr, Locale, PhoneNumber,
        TrustScore,
    },
};
use crate::infrastructure::postgres::models::{PostgresAccountRole, PostgresAccountState};
use shared_kernel::{
    domain::{
        Identifier,
        events::AggregateMetadata,
        value_objects::{AccountId, RegionCode},
    },
    errors::{DomainError, Result},
};

#[derive(Debug, sqlx::FromRow)]
pub struct PostgresAccountRow {
    // --- Champs de Identity ---
    pub account_id: uuid::Uuid,
    pub external_id: String,
    pub email: Option<String>,
    pub email_verified: bool,
    pub phone_number: Option<String>,
    pub phone_verified: bool,
    pub state: PostgresAccountState,
    pub birth_date: Option<chrono::NaiveDate>,
    pub locale: String,
    pub region_code: Option<String>,
    pub version: i64,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub aggregate_updated_at: chrono::DateTime<chrono::Utc>,
    pub last_active_at: Option<chrono::DateTime<chrono::Utc>>,

    // --- Champs de Governance ---
    pub role: PostgresAccountRole,
    pub is_beta_tester: bool,
    pub is_shadowbanned: bool,
    pub trust_score: i32,
    pub moderation_notes: Option<String>,
    pub last_moderation_at: Option<chrono::DateTime<chrono::Utc>>,
    pub last_ip_addr: Option<std::net::IpAddr>,

    // --- Champs de Settings ---
    pub preferences: serde_json::Value,
    pub timezone: String,
    pub push_tokens: Vec<String>,
}

impl PostgresAccountRow {
    pub fn to_domain(self) -> Result<Account> {
        let account_id = AccountId::from_uuid(self.account_id);

        // 1. Reconstruction de Identity
        let identity = AccountIdentity::restore(
            account_id,
            RegionCode::try_new(self.region_code.as_deref().unwrap_or("US"))?,
            ExternalId::try_new(self.external_id)?,
            self.email.map(|e| Email::try_new(e)).transpose()?,
            self.email_verified,
            self.phone_number
                .map(|p| PhoneNumber::try_new(p))
                .transpose()?,
            self.phone_verified,
            AccountState::from(self.state),
            self.birth_date.map(BirthDate::from_raw),
            Locale::try_new(self.locale)?,
            self.created_at,
            self.last_active_at,
        );

        // 2. Reconstruction de Governance
        let governance = AccountGovernance::restore(
            account_id,
            AccountRole::from(self.role),
            self.is_beta_tester,
            self.is_shadowbanned,
            TrustScore::try_new(self.trust_score)?,
            self.last_moderation_at,
            self.moderation_notes,
            self.last_ip_addr.map(IpAddr::from_raw),
        );

        // 3. Reconstruction de Settings
        let preferences: AccountPreferences = serde_json::from_value(self.preferences)
            .map_err(|e| DomainError::Internal(format!("JSON settings error: {}", e)))?;

        let push_tokens = self
            .push_tokens
            .into_iter()
            .map(shared_kernel::domain::value_objects::PushToken::try_new)
            .collect::<Result<Vec<_>>>()?;

        let settings = AccountSettings::restore(
            account_id,
            preferences,
            shared_kernel::domain::value_objects::Timezone::try_new(&self.timezone)?,
            push_tokens,
        );

        // 4. Reconstruction de l'Agrégat complet
        Ok(Account::restore(
            identity,
            governance,
            settings,
            AggregateMetadata::restore(self.version as u64, self.aggregate_updated_at),
        ))
    }
}
