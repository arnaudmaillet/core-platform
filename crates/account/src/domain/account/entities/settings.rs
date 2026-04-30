// crates/account/src/domain/entities/account_settings.rs

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use shared_kernel::{
    domain::{
        entities::Entity,
        value_objects::{AccountId, PushToken, RegionCode, Timezone},
    },
    errors::{DomainError, Result},
};

use crate::domain::{
    account::builders::AccountSettingsBuilder,
    preferences::models::{AppearancePreferences, NotificationPreferences, PrivacyPreferences},
};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct AccountPreferences {
    privacy: PrivacyPreferences,
    notifications: NotificationPreferences,
    appearance: AppearancePreferences,
}

impl AccountPreferences {
    pub fn new(
        privacy: PrivacyPreferences,
        notifications: NotificationPreferences,
        appearance: AppearancePreferences,
    ) -> Self {
        Self {
            privacy,
            notifications,
            appearance,
        }
    }

    pub fn privacy(&self) -> &PrivacyPreferences {
        &self.privacy
    }

    pub fn notifications(&self) -> &NotificationPreferences {
        &self.notifications
    }

    pub fn appearance(&self) -> &AppearancePreferences {
        &self.appearance
    }

    pub(crate) fn update_privacy(&mut self, new_privacy: PrivacyPreferences) -> bool {
        if self.privacy == new_privacy {
            return false;
        }
        self.privacy = new_privacy;
        true
    }

    pub(crate) fn update_notifications(
        &mut self,
        new_notifications: NotificationPreferences,
    ) -> bool {
        if self.notifications == new_notifications {
            return false;
        }
        self.notifications = new_notifications;
        true
    }

    pub(crate) fn update_appearance(&mut self, new_appearance: AppearancePreferences) -> bool {
        if self.appearance == new_appearance {
            return false;
        }
        self.appearance = new_appearance;
        true
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountSettings {
    account_id: AccountId,
    preferences: AccountPreferences,
    timezone: Timezone,
    push_tokens: Vec<PushToken>,
    updated_at: DateTime<Utc>,
}

impl AccountSettings {
    pub fn builder(account_id: AccountId) -> AccountSettingsBuilder {
        AccountSettingsBuilder::new(account_id)
    }

    pub(crate) fn restore(
        account_id: AccountId,
        preferences: AccountPreferences,
        timezone: Timezone,
        push_tokens: Vec<PushToken>,
        updated_at: DateTime<Utc>,
    ) -> Self {
        Self {
            account_id,
            preferences,
            timezone,
            push_tokens,
            updated_at,
        }
    }

    // --- GETTERS ---
    pub fn account_id(&self) -> &AccountId {
        &self.account_id
    }
    pub fn preferences(&self) -> &AccountPreferences {
        &self.preferences
    }
    pub fn timezone(&self) -> &Timezone {
        &self.timezone
    }
    pub fn push_tokens(&self) -> &[PushToken] {
        &self.push_tokens
    }

    // --- MUTATIONS INTERNES (pub(crate)) ---

    pub(crate) fn apply_timezone_update(
        &mut self,
        new_tz: Timezone,
        region: &RegionCode,
    ) -> Result<bool> {
        if self.timezone == new_tz {
            return Ok(false);
        }

        if !new_tz.is_compatible_with(region) {
            return Err(DomainError::Validation {
                field: "timezone".into(),
                reason: format!(
                    "Timezone '{}' is inconsistent with region '{}'",
                    new_tz, region
                ),
            });
        }

        self.timezone = new_tz;
        Ok(true)
    }

    pub(crate) fn apply_push_token_add(&mut self, token: PushToken) -> bool {
        if self.push_tokens.contains(&token) {
            return false;
        }

        if self.push_tokens.len() >= 10 {
            self.push_tokens.remove(0);
        }

        self.push_tokens.push(token);
        true
    }

    pub(crate) fn apply_push_token_remove(&mut self, token: &PushToken) -> bool {
        let original_len = self.push_tokens.len();
        self.push_tokens.retain(|t| t != token);
        self.push_tokens.len() != original_len
    }

    pub(crate) fn apply_notifications_update(
        &mut self,
        new_prefs: NotificationPreferences,
    ) -> bool {
        self.preferences.update_notifications(new_prefs)
    }

    pub(crate) fn apply_appearance_update(&mut self, new_prefs: AppearancePreferences) -> bool {
        self.preferences.update_appearance(new_prefs)
    }

    pub(crate) fn apply_privacy_update(&mut self, new_prefs: PrivacyPreferences) -> bool {
        self.preferences.update_privacy(new_prefs)
    }
}

impl Entity for AccountSettings {
    type Id = AccountId;

    fn id(&self) -> &Self::Id {
        &self.account_id
    }

    fn entity_name() -> &'static str {
        "AccountSettings"
    }
    fn map_constraint_to_field(constraint: &str) -> &'static str {
        match constraint {
            "account_settings_pkey" => "account_id",
            _ => "settings",
        }
    }

    fn updated_at(&self) -> DateTime<Utc> {
        self.updated_at
    }
}
