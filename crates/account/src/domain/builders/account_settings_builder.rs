// crates/account/src/domain/builders/account_settings_builder.rs

use chrono::{DateTime, Utc};
use shared_kernel::domain::events::AggregateMetadata;
use shared_kernel::domain::value_objects::{PushToken, RegionCode, Timezone, AccountId};
use crate::domain::entities::{
    AccountSettings, PrivacySettings, NotificationSettings, AppearanceSettings
};

pub struct AccountSettingsBuilder {
    account_id: AccountId,
    region_code: RegionCode,
    timezone: Option<Timezone>,
    privacy: Option<PrivacySettings>,
    notifications: Option<NotificationSettings>,
    appearance: Option<AppearanceSettings>,
    push_tokens: Vec<PushToken>,
    // Ajout de la version pour le suivi technique
    version: i32,
}

impl AccountSettingsBuilder {
    pub fn new(account_id: AccountId, region_code: RegionCode) -> Self {
        Self {
            account_id,
            region_code,
            timezone: None,
            privacy: None,
            notifications: None,
            appearance: None,
            push_tokens: Vec::new(),
            version: 1,
        }
    }

    /// CHEMIN 2 : RESTAURATION (Haute performance depuis la DB)
    /// On injecte directement la version stockée en base.
    #[allow(clippy::too_many_arguments)]
    pub fn restore(
        account_id: AccountId,
        region_code: RegionCode,
        privacy: PrivacySettings,
        notifications: NotificationSettings,
        appearance: AppearanceSettings,
        timezone: Timezone,
        push_tokens: Vec<PushToken>,
        updated_at: DateTime<Utc>,
        version: i32,
    ) -> AccountSettings {
        AccountSettings {
            account_id,
            region_code,
            privacy,
            notifications,
            appearance,
            timezone,
            push_tokens,
            updated_at,
            metadata: AggregateMetadata::restore(version),
        }
    }

    // --- SETTERS (Chemin Création) ---

    pub fn with_timezone(mut self, timezone: Timezone) -> Self {
        self.timezone = Some(timezone);
        self
    }

    pub fn with_privacy(mut self, privacy: PrivacySettings) -> Self {
        self.privacy = Some(privacy);
        self
    }

    pub fn with_notifications(mut self, notifications: NotificationSettings) -> Self {
        self.notifications = Some(notifications);
        self
    }

    pub fn with_appearance(mut self, appearance: AppearanceSettings) -> Self {
        self.appearance = Some(appearance);
        self
    }

    pub fn with_initial_push_token(mut self, token: PushToken) -> Self {
        self.push_tokens.push(token);
        self
    }

    /// Finalise pour une CRÉATION
    pub fn build(self) -> AccountSettings {
        let now = Utc::now();

        AccountSettings {
            account_id: self.account_id,
            region_code: self.region_code,
            timezone: self.timezone.unwrap_or_else(|| {
                Timezone::new_unchecked("UTC")
            }),
            privacy: self.privacy.unwrap_or_default(),
            notifications: self.notifications.unwrap_or_default(),
            appearance: self.appearance.unwrap_or_default(),
            push_tokens: self.push_tokens,
            updated_at: now,
            metadata: AggregateMetadata::new(self.version),
        }
    }
}