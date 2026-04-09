// crates/account/src/domain/builders/account_settings_builder.rs

use crate::domain::account::entities::{AccountSettings, AccountPreferences};
use crate::domain::preferences::models::{AppearancePreferences, NotificationPreferences, PrivacyPreferences};
use chrono::{DateTime, Utc};
use shared_kernel::domain::events::AggregateMetadata;
use shared_kernel::domain::value_objects::{AccountId, PushToken, RegionCode, Timezone};

pub struct AccountSettingsBuilder {
    account_id: AccountId,
    timezone: Timezone,
    privacy: PrivacyPreferences,
    notifications: NotificationPreferences,
    appearance: AppearancePreferences,
    push_tokens: Vec<PushToken>,
    version: u64,
}

impl AccountSettingsBuilder {
    pub(crate) fn new(account_id: AccountId) -> Self {
        Self {
            account_id,
            timezone: Timezone::from_raw("UTC"),
            privacy: PrivacyPreferences::builder().build(),
            notifications: NotificationPreferences::builder().build(),
            appearance: AppearancePreferences::builder().build(),
            push_tokens: Vec::new(),
            version: 1,
        }
    }

    /// CHEMIN 2 : RESTAURATION (Depuis la DB)
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn restore(
        account_id: AccountId,
        preferences: AccountPreferences,
        timezone: Timezone,
        push_tokens: Vec<PushToken>,
        updated_at: DateTime<Utc>,
        version: u64,
    ) -> AccountSettings {
        AccountSettings::restore(
            account_id,
            preferences,
            timezone,
            push_tokens,
            updated_at,
            AggregateMetadata::restore(version),
        )
    }

    // --- SETTERS FLUIDES ---

    pub fn with_timezone(mut self, timezone: Timezone) -> Self {
        self.timezone = timezone;
        self
    }

    pub fn with_privacy(mut self, privacy: PrivacyPreferences) -> Self {
        self.privacy = privacy;
        self
    }

    pub fn with_notifications(mut self, notifications: NotificationPreferences) -> Self {
        self.notifications = notifications;
        self
    }

    pub fn with_appearance(mut self, appearance: AppearancePreferences) -> Self {
        self.appearance = appearance;
        self
    }

    pub fn with_initial_push_tokens(mut self, tokens: Vec<PushToken>) -> Self {
        self.push_tokens = tokens;
        self
    }

    /// Finalise pour une CRÉATION
    pub fn build(self) -> AccountSettings {
        let now = Utc::now();

        let metadata = AggregateMetadata::new(self.version);
        let preferences = AccountPreferences::new(
            self.privacy,
            self.notifications,
            self.appearance,
        );

        // Centralisation via restore
        AccountSettings::restore(
            self.account_id,
            preferences,
            self.timezone,
            self.push_tokens,
            now,
            metadata,
        )
    }
}
