// crates/account/src/domain/builders/account_settings_builder.rs


use crate::domain::account::entities::{AccountSettings, AccountPreferences};
use crate::domain::preferences::models::{AppearancePreferences, NotificationPreferences, PrivacyPreferences};
use chrono::{DateTime, Utc};
use shared_kernel::domain::events::AggregateMetadata;
use shared_kernel::domain::value_objects::{AccountId, PushToken, RegionCode, Timezone};

pub struct AccountSettingsBuilder {
    account_id: AccountId,
    region_code: RegionCode,
    timezone: Option<Timezone>,
    privacy: Option<PrivacyPreferences>,
    notifications: Option<NotificationPreferences>,
    appearance: Option<AppearancePreferences>,
    push_tokens: Vec<PushToken>,
    version: u64,
}

impl AccountSettingsBuilder {
    pub(crate) fn new(account_id: AccountId, region_code: RegionCode) -> Self {
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

    /// CHEMIN 2 : RESTAURATION (Depuis la DB)
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn restore(
        account_id: AccountId,
        region_code: RegionCode,
        preferences: AccountPreferences,
        timezone: Timezone,
        push_tokens: Vec<PushToken>,
        updated_at: DateTime<Utc>,
        version: u64,
    ) -> AccountSettings {
        AccountSettings::restore(
            account_id,
            region_code,
            preferences,
            timezone,
            push_tokens,
            updated_at,
            AggregateMetadata::restore(version),
        )
    }

    // --- SETTERS FLUIDES ---

    pub fn with_timezone(mut self, timezone: Timezone) -> Self {
        self.timezone = Some(timezone);
        self
    }

    pub fn with_optional_timezone(mut self, timezone: Option<Timezone>) -> Self {
        self.timezone = timezone;
        self
    }

    pub fn with_privacy(mut self, privacy: PrivacyPreferences) -> Self {
        self.privacy = Some(privacy);
        self
    }

    pub fn with_optional_privacy(mut self, privacy: Option<PrivacyPreferences>) -> Self {
        self.privacy = privacy;
        self
    }

    pub fn with_notifications(mut self, notifications: NotificationPreferences) -> Self {
        self.notifications = Some(notifications);
        self
    }

    pub fn with_optional_notifications(
        mut self,
        notifications: Option<NotificationPreferences>,
    ) -> Self {
        self.notifications = notifications;
        self
    }

    pub fn with_appearance(mut self, appearance: AppearancePreferences) -> Self {
        self.appearance = Some(appearance);
        self
    }

    pub fn with_optional_appearance(mut self, appearance: Option<AppearancePreferences>) -> Self {
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
        let timezone = self.timezone.unwrap_or_else(|| Timezone::from_raw("UTC"));

        let preferences = AccountPreferences::new(
            self.privacy.unwrap_or_default(),
            self.notifications.unwrap_or_default(),
            self.appearance.unwrap_or_default(),
        );

        // Centralisation via restore
        AccountSettings::restore(
            self.account_id,
            self.region_code,
            preferences,
            timezone,
            self.push_tokens,
            now,
            AggregateMetadata::new(self.version),
        )
    }
}
