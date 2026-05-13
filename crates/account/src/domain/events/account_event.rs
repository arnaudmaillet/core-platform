// crates/account/src/domain/entities/account_event.rs

use crate::domain::preferences::models::{
    AppearancePreferences, NotificationPreferences, PrivacyPreferences,
};
use crate::domain::value_objects::{AccountRole, IpAddr, Locale, TrustDelta, TrustScore};
use crate::value_objects::{BetaTier, BirthDate};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use shared_kernel::domain::events::DomainEvent;
use shared_kernel::types::{
    AccountId, Email, PhoneNumber, PushToken, RegionCode, SubId, Timezone,
};
use std::borrow::Cow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum AccountEvent {
    // --- IDENTITY & SECURITY EVENTS ---
    AccountRegistered {
        account_id: AccountId,
        region: RegionCode,
        email: Option<Email>,
        phone: Option<PhoneNumber>,
        sub_id: Option<SubId>,
        locale: Locale,
        ip_addr: IpAddr,
        occurred_at: DateTime<Utc>,
    },
    SubIdentityLinked {
        account_id: AccountId,
        region: RegionCode,
        old_sub_id: Option<SubId>,
        new_sub_id: SubId,
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
    BirthDateChanged {
        account_id: AccountId,
        region: RegionCode,
        old_birth_date: Option<BirthDate>,
        new_birth_date: BirthDate,
        occurred_at: DateTime<Utc>,
    },
    LocaleUpdated {
        account_id: AccountId,
        region: RegionCode,
        new_locale: Locale,
        occurred_at: DateTime<Utc>,
    },

    // --- SYSTEM & MODERATION EVENTS ---
    BetaTierChanged {
        account_id: AccountId,
        region: RegionCode,
        old_tier: BetaTier,
        new_tier: BetaTier,
        occurred_at: DateTime<Utc>,
    },
    TrustScoreAdjusted {
        id: Uuid,
        account_id: AccountId,
        region: RegionCode,
        delta: TrustDelta,
        new_score: TrustScore,
        reason: String,
        occurred_at: DateTime<Utc>,
    },
    ShadowbanUpdated {
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
        reason: String,
        occurred_at: DateTime<Utc>,
    },
    AccountActivated {
        account_id: AccountId,
        region: RegionCode,
        reason: String,
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
        reason: String,
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
        reason: String,
        occurred_at: DateTime<Utc>,
    },

    // --- SETTINGS EVENTS ---
    NotificationsPreferencesUpdated {
        account_id: AccountId,
        region: RegionCode,
        new_preferences: NotificationPreferences,
        occurred_at: DateTime<Utc>,
    },
    AppearancePreferencesUpdated {
        account_id: AccountId,
        region: RegionCode,
        new_preferences: AppearancePreferences,
        occurred_at: DateTime<Utc>,
    },
    PrivacyPreferencesUpdated {
        account_id: AccountId,
        region: RegionCode,
        new_preferences: PrivacyPreferences,
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
    TimezoneUpdated {
        account_id: AccountId,
        region: RegionCode,
        new_timezone: Timezone,
        occurred_at: DateTime<Utc>,
    },
}

impl AccountEvent {
    // Identity & Security
    pub const REGISTERED: &'static str = "account.identity.registered";
    pub const EXTERNAL_LINKED: &'static str = "account.identity.external_linked";
    pub const EMAIL_CHANGED: &'static str = "account.identity.email_changed";
    pub const PHONE_NUMBER_CHANGED: &'static str = "account.identity.phone_number_changed";
    pub const BIRTH_DATE_CHANGED: &'static str = "account.identity.birth_date_changed";
    pub const LOCALE_UPDATED: &'static str = "account.identity.locale_updated";

    // Metadata & System
    pub const BETA_TIER_CHANGED: &'static str = "account.metadata.beta_tier_changed";
    pub const TRUST_SCORE_ADJUSTED: &'static str = "account.metadata.trust_score_adjusted";
    pub const SHADOWBAN_UPDATED: &'static str = "account.metadata.shadowban_updated";
    pub const ROLE_CHANGED: &'static str = "account.metadata.role_changed";
    pub const REGION_CHANGED: &'static str = "account.system.region_changed";

    // Lifecycle & Moderation
    pub const DEACTIVATED: &'static str = "account.deactivated";
    pub const ACTIVATED: &'static str = "account.activated";
    pub const BANNED: &'static str = "account.banned";
    pub const UNBANNED: &'static str = "account.unbanned";
    pub const SUSPENDED: &'static str = "account.suspended";
    pub const UNSUSPENDED: &'static str = "account.unsuspended";

    // Settings
    pub const NOTIFICATIONS_PREFS_UPDATED: &'static str =
        "account.settings.notifications_preferences_updated";
    pub const APPEARANCE_PREFS_UPDATED: &'static str =
        "account.settings.appearance_preferences_updated";
    pub const PRIVACY_PREFS_UPDATED: &'static str = "account.settings.privacy_preferences_updated";
    pub const PUSH_TOKEN_ADDED: &'static str = "account.settings.push_token_added";
    pub const PUSH_TOKEN_REMOVED: &'static str = "account.settings.push_token_removed";
    pub const TIMEZONE_UPDATED: &'static str = "account.settings.timezone_updated";
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

    fn event_name(&self) -> Cow<'_, str> {
        let s = match self {
            Self::AccountRegistered { .. } => Self::REGISTERED,
            Self::SubIdentityLinked { .. } => Self::EXTERNAL_LINKED,
            Self::EmailChanged { .. } => Self::EMAIL_CHANGED,
            Self::PhoneNumberChanged { .. } => Self::PHONE_NUMBER_CHANGED,
            Self::BirthDateChanged { .. } => Self::BIRTH_DATE_CHANGED,
            Self::LocaleUpdated { .. } => Self::LOCALE_UPDATED,
            Self::BetaTierChanged { .. } => Self::BETA_TIER_CHANGED,
            Self::TrustScoreAdjusted { .. } => Self::TRUST_SCORE_ADJUSTED,
            Self::ShadowbanUpdated { .. } => Self::SHADOWBAN_UPDATED,
            Self::AccountRoleChanged { .. } => Self::ROLE_CHANGED,
            Self::AccountRegionChanged { .. } => Self::REGION_CHANGED,
            Self::AccountDeactivated { .. } => Self::DEACTIVATED,
            Self::AccountActivated { .. } => Self::ACTIVATED,
            Self::AccountBanned { .. } => Self::BANNED,
            Self::AccountUnbanned { .. } => Self::UNBANNED,
            Self::AccountSuspended { .. } => Self::SUSPENDED,
            Self::AccountUnsuspended { .. } => Self::UNSUSPENDED,
            Self::NotificationsPreferencesUpdated { .. } => Self::NOTIFICATIONS_PREFS_UPDATED,
            Self::AppearancePreferencesUpdated { .. } => Self::APPEARANCE_PREFS_UPDATED,
            Self::PrivacyPreferencesUpdated { .. } => Self::PRIVACY_PREFS_UPDATED,
            Self::PushTokenAdded { .. } => Self::PUSH_TOKEN_ADDED,
            Self::PushTokenRemoved { .. } => Self::PUSH_TOKEN_REMOVED,
            Self::TimezoneUpdated { .. } => Self::TIMEZONE_UPDATED,
        };
        Cow::Borrowed(s)
    }

    fn aggregate_type(&self) -> Cow<'_, str> {
        Cow::Borrowed("account")
    }

    fn aggregate_id(&self) -> String {
        // Pattern matching simplifié pour tous les types portant un account_id
        match self {
            Self::AccountRegistered { account_id, .. }
            | Self::SubIdentityLinked { account_id, .. }
            | Self::EmailChanged { account_id, .. }
            | Self::PhoneNumberChanged { account_id, .. }
            | Self::BirthDateChanged { account_id, .. }
            | Self::LocaleUpdated { account_id, .. }
            | Self::BetaTierChanged { account_id, .. }
            | Self::TrustScoreAdjusted { account_id, .. }
            | Self::ShadowbanUpdated { account_id, .. }
            | Self::AccountRoleChanged { account_id, .. }
            | Self::AccountRegionChanged { account_id, .. }
            | Self::AccountDeactivated { account_id, .. }
            | Self::AccountActivated { account_id, .. }
            | Self::AccountBanned { account_id, .. }
            | Self::AccountUnbanned { account_id, .. }
            | Self::AccountSuspended { account_id, .. }
            | Self::AccountUnsuspended { account_id, .. }
            | Self::NotificationsPreferencesUpdated { account_id, .. }
            | Self::AppearancePreferencesUpdated { account_id, .. }
            | Self::PrivacyPreferencesUpdated { account_id, .. }
            | Self::PushTokenAdded { account_id, .. }
            | Self::PushTokenRemoved { account_id, .. }
            | Self::TimezoneUpdated { account_id, .. } => account_id.to_string(),
        }
    }

    fn occurred_at(&self) -> DateTime<Utc> {
        match self {
            Self::AccountRegistered { occurred_at, .. }
            | Self::SubIdentityLinked { occurred_at, .. }
            | Self::EmailChanged { occurred_at, .. }
            | Self::PhoneNumberChanged { occurred_at, .. }
            | Self::BirthDateChanged { occurred_at, .. }
            | Self::LocaleUpdated { occurred_at, .. }
            | Self::BetaTierChanged { occurred_at, .. }
            | Self::TrustScoreAdjusted { occurred_at, .. }
            | Self::ShadowbanUpdated { occurred_at, .. }
            | Self::AccountRoleChanged { occurred_at, .. }
            | Self::AccountRegionChanged { occurred_at, .. }
            | Self::AccountDeactivated { occurred_at, .. }
            | Self::AccountActivated { occurred_at, .. }
            | Self::AccountBanned { occurred_at, .. }
            | Self::AccountUnbanned { occurred_at, .. }
            | Self::AccountSuspended { occurred_at, .. }
            | Self::AccountUnsuspended { occurred_at, .. }
            | Self::NotificationsPreferencesUpdated { occurred_at, .. }
            | Self::AppearancePreferencesUpdated { occurred_at, .. }
            | Self::PrivacyPreferencesUpdated { occurred_at, .. }
            | Self::PushTokenAdded { occurred_at, .. }
            | Self::PushTokenRemoved { occurred_at, .. }
            | Self::TimezoneUpdated { occurred_at, .. } => *occurred_at,
        }
    }

    fn payload(&self) -> Value {
        serde_json::to_value(self).unwrap_or(Value::Null)
    }
}
