// crates/account/src/domain/entities/account_event.rs

use crate::domain::value_objects::{AccountRole, Email, ExternalId, Locale, PhoneNumber};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use shared_kernel::domain::events::DomainEvent;
use shared_kernel::domain::value_objects::{AccountId, PushToken, RegionCode, Timezone, Username};
use std::borrow::Cow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum AccountEvent {
    // --- IDENTITY & SECURITY EVENTS ---
    AccountCreated {
        account_id: AccountId,
        region: RegionCode,
        username: Username,
        display_name: String,
        occurred_at: DateTime<Utc>,
    },
    ExternalIdentityLinked {
        account_id: AccountId,
        region: RegionCode,
        external_id: ExternalId,
        occurred_at: DateTime<Utc>,
    },
    UsernameChanged {
        account_id: AccountId,
        region: RegionCode,
        old_username: Username,
        new_username: Username,
        occurred_at: DateTime<Utc>,
    },
    EmailChanged {
        account_id: AccountId,
        region: RegionCode,
        old_email: Option<Email>,
        new_email: Email,
        occurred_at: DateTime<Utc>,
    },
    PhoneNumberChanged {
        account_id: AccountId,
        region: RegionCode,
        old_phone_number: Option<PhoneNumber>,
        new_phone_number: PhoneNumber,
        occurred_at: DateTime<Utc>,
    },
    EmailVerified {
        account_id: AccountId,
        region: RegionCode,
        occurred_at: DateTime<Utc>,
    },
    PhoneVerified {
        account_id: AccountId,
        region: RegionCode,
        occurred_at: DateTime<Utc>,
    },
    BirthDateChanged {
        account_id: AccountId,
        region: RegionCode,
        occurred_at: DateTime<Utc>,
    },
    LocaleChanged {
        account_id: AccountId,
        region: RegionCode,
        new_locale: Locale,
        occurred_at: DateTime<Utc>,
    },

    // --- SYSTEM & MODERATION EVENTS ---
    BetaStatusChanged {
        account_id: AccountId,
        region: RegionCode,
        is_beta_tester: bool,
        occurred_at: DateTime<Utc>,
    },
    TrustScoreAdjusted {
        id: Uuid,
        account_id: AccountId,
        region: RegionCode,
        delta: i32,
        new_score: i32,
        reason: String,
        occurred_at: DateTime<Utc>,
    },
    ShadowbanStatusChanged {
        account_id: AccountId,
        region: RegionCode,
        is_shadowbanned: bool,
        reason: String,
        occurred_at: DateTime<Utc>,
    },
    AccountRoleChanged {
        account_id: AccountId,
        region: RegionCode,
        old_role: AccountRole,
        new_role: AccountRole,
        reason: String,
        occurred_at: DateTime<Utc>,
    },

    // --- HYPERSCALE / SHARDING EVENTS ---
    AccountRegionChanged {
        account_id: AccountId,
        old_region: RegionCode,
        new_region: RegionCode,
        occurred_at: DateTime<Utc>,
    },

    // --- STATE & MODERATION ---
    AccountDeactivated {
        account_id: AccountId,
        region: RegionCode,
        occurred_at: DateTime<Utc>,
    },
    AccountReactivated {
        account_id: AccountId,
        region: RegionCode,
        occurred_at: DateTime<Utc>,
    },
    AccountBanned {
        account_id: AccountId,
        region: RegionCode,
        reason: String,
        occurred_at: DateTime<Utc>,
    },
    AccountUnbanned {
        account_id: AccountId,
        region: RegionCode,
        occurred_at: DateTime<Utc>,
    },
    AccountSuspended {
        account_id: AccountId,
        region: RegionCode,
        reason: String,
        occurred_at: DateTime<Utc>,
    },
    AccountUnsuspended {
        account_id: AccountId,
        region: RegionCode,
        occurred_at: DateTime<Utc>,
    },

    // --- SETTINGS EVENTS ---
    AccountSettingsUpdated {
        account_id: AccountId,
        region: RegionCode,
        occurred_at: DateTime<Utc>,
    },

    /// Spécifique pour le routage des notifications
    PushTokenAdded {
        account_id: AccountId,
        region: RegionCode,
        token: PushToken,
        occurred_at: DateTime<Utc>,
    },
    PushTokenRemoved {
        account_id: AccountId,
        region: RegionCode,
        token: PushToken,
        occurred_at: DateTime<Utc>,
    },
    TimezoneChanged {
        account_id: AccountId,
        region: RegionCode,
        new_timezone: Timezone,
        occurred_at: DateTime<Utc>,
    },
}

impl DomainEvent for AccountEvent {
    fn event_id(&self) -> Uuid {
        match self {
            // Pour les ajustements de score, on utilise l'ID déterministe
            // qui vient de la commande (action_id).
            Self::TrustScoreAdjusted { id, .. } => *id,
            _ => Uuid::now_v7(),
        }
    }

    fn event_type(&self) -> Cow<'_, str> {
        let s = match self {
            Self::AccountCreated { .. } => "account.created",
            Self::ExternalIdentityLinked { .. } => "account.external_linked",
            Self::UsernameChanged { .. } => "account.username_changed",
            Self::EmailChanged { .. } => "account.email_changed",
            Self::PhoneNumberChanged { .. } => "account.phone_number_changed",
            Self::EmailVerified { .. } => "account.email_verified",
            Self::PhoneVerified { .. } => "account.phone_verified",
            Self::BirthDateChanged { .. } => "account.birth_date_changed",
            Self::LocaleChanged { .. } => "account.locale_changed",
            Self::BetaStatusChanged { .. } => "account.metadata.beta_status_changed",
            Self::TrustScoreAdjusted { .. } => "account.metadata.trust_score_adjusted",
            Self::ShadowbanStatusChanged { .. } => "account.metadata.shadowban_status_changed",
            Self::AccountRoleChanged { .. } => "account.metadata.role_changed",
            Self::AccountRegionChanged { .. } => "account.system.region_changed",
            Self::AccountDeactivated { .. } => "account.deactivated",
            Self::AccountReactivated { .. } => "account.reactivated",
            Self::AccountBanned { .. } => "account.banned",
            Self::AccountUnbanned { .. } => "account.unbanned",
            Self::AccountSuspended { .. } => "account.suspended",
            Self::AccountUnsuspended { .. } => "account.unsuspended",
            Self::AccountSettingsUpdated { .. } => "account.settings.updated",
            Self::PushTokenAdded { .. } => "account.settings.push_token_added",
            Self::PushTokenRemoved { .. } => "account.settings.push_token_removed",
            Self::TimezoneChanged { .. } => "account.settings.timezone_changed",
        };
        Cow::Borrowed(s)
    }

    fn region_code(&self) -> RegionCode {
        match self {
            Self::AccountCreated { region, .. }
            | Self::ExternalIdentityLinked { region, .. }
            | Self::UsernameChanged { region, .. }
            | Self::EmailChanged { region, .. }
            | Self::PhoneNumberChanged { region, .. }
            | Self::EmailVerified { region, .. }
            | Self::PhoneVerified { region, .. }
            | Self::BirthDateChanged { region, .. }
            | Self::LocaleChanged { region, .. }
            | Self::BetaStatusChanged { region, .. }
            | Self::TrustScoreAdjusted { region, .. }
            | Self::ShadowbanStatusChanged { region, .. }
            | Self::AccountRoleChanged { region, .. }
            | Self::AccountDeactivated { region, .. }
            | Self::AccountReactivated { region, .. }
            | Self::AccountBanned { region, .. }
            | Self::AccountUnbanned { region, .. }
            | Self::AccountSuspended { region, .. }
            | Self::AccountUnsuspended { region, .. }
            | Self::AccountSettingsUpdated { region, .. }
            | Self::PushTokenAdded { region, .. }
            | Self::PushTokenRemoved { region, .. }
            | Self::TimezoneChanged { region, .. } => region.clone(),
            Self::AccountRegionChanged { new_region, .. } => new_region.clone(),
        }
    }

    fn aggregate_type(&self) -> Cow<'_, str> {
        Cow::Borrowed("account")
    }

    fn aggregate_id(&self) -> String {
        // Pattern matching simplifié pour tous les types portant un account_id
        match self {
            Self::AccountCreated { account_id, .. }
            | Self::ExternalIdentityLinked { account_id, .. }
            | Self::UsernameChanged { account_id, .. }
            | Self::EmailChanged { account_id, .. }
            | Self::PhoneNumberChanged { account_id, .. }
            | Self::EmailVerified { account_id, .. }
            | Self::PhoneVerified { account_id, .. }
            | Self::BirthDateChanged { account_id, .. }
            | Self::LocaleChanged { account_id, .. }
            | Self::BetaStatusChanged { account_id, .. }
            | Self::TrustScoreAdjusted { account_id, .. }
            | Self::ShadowbanStatusChanged { account_id, .. }
            | Self::AccountRoleChanged { account_id, .. }
            | Self::AccountRegionChanged { account_id, .. }
            | Self::AccountDeactivated { account_id, .. }
            | Self::AccountReactivated { account_id, .. }
            | Self::AccountBanned { account_id, .. }
            | Self::AccountUnbanned { account_id, .. }
            | Self::AccountSuspended { account_id, .. }
            | Self::AccountUnsuspended { account_id, .. }
            | Self::AccountSettingsUpdated { account_id, .. }
            | Self::PushTokenAdded { account_id, .. }
            | Self::PushTokenRemoved { account_id, .. }
            | Self::TimezoneChanged { account_id, .. } => account_id.to_string(),
        }
    }

    fn occurred_at(&self) -> DateTime<Utc> {
        match self {
            Self::AccountCreated { occurred_at, .. }
            | Self::ExternalIdentityLinked { occurred_at, .. }
            | Self::UsernameChanged { occurred_at, .. }
            | Self::EmailChanged { occurred_at, .. }
            | Self::PhoneNumberChanged { occurred_at, .. }
            | Self::EmailVerified { occurred_at, .. }
            | Self::PhoneVerified { occurred_at, .. }
            | Self::BirthDateChanged { occurred_at, .. }
            | Self::LocaleChanged { occurred_at, .. }
            | Self::BetaStatusChanged { occurred_at, .. }
            | Self::TrustScoreAdjusted { occurred_at, .. }
            | Self::ShadowbanStatusChanged { occurred_at, .. }
            | Self::AccountRoleChanged { occurred_at, .. }
            | Self::AccountRegionChanged { occurred_at, .. }
            | Self::AccountDeactivated { occurred_at, .. }
            | Self::AccountReactivated { occurred_at, .. }
            | Self::AccountBanned { occurred_at, .. }
            | Self::AccountUnbanned { occurred_at, .. }
            | Self::AccountSuspended { occurred_at, .. }
            | Self::AccountUnsuspended { occurred_at, .. }
            | Self::AccountSettingsUpdated { occurred_at, .. }
            | Self::PushTokenAdded { occurred_at, .. }
            | Self::PushTokenRemoved { occurred_at, .. }
            | Self::TimezoneChanged { occurred_at, .. } => *occurred_at,
        }
    }

    fn payload(&self) -> Value {
        serde_json::to_value(self).unwrap_or(Value::Null)
    }
}
