use crate::domain::account::builders::AccountSettingsBuilder;
use crate::domain::events::AccountEvent;
use crate::domain::preferences::models::{
    AppearancePreferences, NotificationPreferences, PrivacyPreferences,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use shared_kernel::domain::Identifier;
use shared_kernel::domain::entities::EntityMetadata;
use shared_kernel::domain::events::{AggregateMetadata, AggregateRoot};
use shared_kernel::domain::value_objects::{AccountId, PushToken, RegionCode, Timezone};
use shared_kernel::errors::{DomainError, Result};

/// Cette struct représente exactement le contenu de la colonne JSONB 'settings'
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct AccountPreferences {
    privacy: PrivacyPreferences,
    notifications: NotificationPreferences,
    appearance: AppearancePreferences,
}

impl AccountPreferences {
    /// Constructeur explicite
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

    // --- GETTERS (Accès en lecture seule) ---

    pub fn privacy(&self) -> &PrivacyPreferences {
        &self.privacy
    }

    pub fn notifications(&self) -> &NotificationPreferences {
        &self.notifications
    }

    pub fn appearance(&self) -> &AppearancePreferences {
        &self.appearance
    }

    // --- SETTERS / UPDATERS (Accès en écriture avec logique) ---

    /// Met à jour la confidentialité et retourne true si une modification a eu lieu
    pub fn update_privacy(&mut self, new_privacy: PrivacyPreferences) -> bool {
        if self.privacy == new_privacy {
            return false;
        }
        self.privacy = new_privacy;
        true
    }

    pub fn update_notifications(&mut self, new_notifications: NotificationPreferences) -> bool {
        if self.notifications == new_notifications {
            return false;
        }
        self.notifications = new_notifications;
        true
    }

    pub fn update_appearance(&mut self, new_appearance: AppearancePreferences) -> bool {
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
    metadata: AggregateMetadata,
}

impl AccountSettings {
    /// Point d'entrée pour le Builder
    pub fn builder(account_id: AccountId) -> AccountSettingsBuilder {
        AccountSettingsBuilder::new(account_id)
    }
    /// Utilisé par le Builder et le Repository
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn restore(
        account_id: AccountId,
        preferences: AccountPreferences,
        timezone: Timezone,
        push_tokens: Vec<PushToken>,
        updated_at: DateTime<Utc>,
        metadata: AggregateMetadata,
    ) -> Self {
        Self {
            account_id,
            preferences,
            timezone,
            push_tokens,
            updated_at,
            metadata,
        }
    }

    // --- GETTERS PUBLICS ---

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
    pub fn updated_at(&self) -> DateTime<Utc> {
        self.updated_at
    }

    /// Met à jour la timezone avec un événement spécifique
    pub fn update_timezone(&mut self, new_tz: Timezone, region: &RegionCode) -> Result<bool> {
        if self.timezone == new_tz {
            return Ok(false);
        }

        if !new_tz.is_compatible_with(region) {
        return Err(DomainError::Validation {
            field: "timezone".into(),
            reason: format!(
                "Timezone '{}' is geographically inconsistent with region '{}'",
                new_tz, region
            ),
        });
    }

        self.timezone = new_tz;
        self.apply_change();

        self.push_event(Box::new(AccountEvent::TimezoneUpdated {
            account_id: self.account_id.clone(),
            new_timezone: self.timezone.clone(),
            occurred_at: self.updated_at,
        }));

        Ok(true)
    }

    pub fn set_timezone_raw(&mut self, new_tz: Timezone) -> bool {
        if self.timezone == new_tz {
            return false;
        }
        self.timezone = new_tz;
        self.apply_change();
        true
    }

    /// Ajoute un token avec événement spécifique et rotation FIFO
    pub fn add_push_token(&mut self, token: PushToken) -> Result<bool> {
        if self.push_tokens.contains(&token) {
            return Ok(false);
        }

        // Rotation FIFO (Max 10 tokens par utilisateur pour limiter la taille du champ)
        if self.push_tokens.len() >= 10 {
            self.push_tokens.remove(0);
        }

        self.push_tokens.push(token.clone());
        self.apply_change();

        self.push_event(Box::new(AccountEvent::PushTokenAdded {
            account_id: self.account_id.clone(),
            token,
            occurred_at: self.updated_at,
        }));

        Ok(true)
    }

    /// Supprime un token (ex: au logout) avec événement spécifique
    pub fn remove_push_token(&mut self, token: &PushToken) -> Result<bool> {
        let original_len = self.push_tokens.len();
        self.push_tokens.retain(|t| t != token);

        if self.push_tokens.len() == original_len {
            return Ok(false);
        }

        self.apply_change();

        self.push_event(Box::new(AccountEvent::PushTokenRemoved {
            account_id: self.account_id.clone(),
            token: token.clone(),
            occurred_at: self.updated_at,
        }));

        Ok(true)
    }

    pub fn update_notifications_preferences(
        &mut self,
        new_prefs: NotificationPreferences,
    ) -> Result<bool> {
        if !self.preferences.update_notifications(new_prefs.clone()) {
            return Ok(false);
        }
        self.apply_change();

        self.push_event(Box::new(AccountEvent::NotificationsPreferencesUpdated {
            account_id: self.account_id.clone(),
            new_preferences: new_prefs,
            occurred_at: self.updated_at,
        }));

        Ok(true)
    }

    pub fn update_appearance_preferences(
        &mut self,
        new_prefs: AppearancePreferences,
    ) -> Result<bool> {
        if !self.preferences.update_appearance(new_prefs.clone()) {
            return Ok(false);
        }
        self.apply_change();

        self.push_event(Box::new(AccountEvent::AppearancePreferencesUpdated {
            account_id: self.account_id.clone(),
            new_preferences: new_prefs,
            occurred_at: self.updated_at,
        }));

        Ok(true)
    }

    pub fn update_privacy_preferences(&mut self, new_prefs: PrivacyPreferences) -> Result<bool> {
        if !self.preferences.update_privacy(new_prefs.clone()) {
            return Ok(false);
        }
        self.apply_change();

        self.push_event(Box::new(AccountEvent::PrivacyPreferencesUpdated {
            account_id: self.account_id.clone(),
            new_preferences: new_prefs,
            occurred_at: self.updated_at,
        }));

        Ok(true)
    }

    // --- LOGIQUE DE VERSIONING ---

    fn apply_change(&mut self) {
        self.increment_version(); // Méthode de AggregateRoot
        self.updated_at = Utc::now();
    }
}

impl EntityMetadata for AccountSettings {
    fn entity_name() -> &'static str {
        "AccountSettings"
    }

    fn map_constraint_to_field(constraint: &str) -> &'static str {
        match constraint {
            "account_settings_pkey" => "account_id",
            _ => "settings",
        }
    }
}

impl AggregateRoot for AccountSettings {
    fn id(&self) -> String {
        self.account_id.as_string()
    }
    fn metadata(&self) -> &AggregateMetadata {
        &self.metadata
    }
    fn metadata_mut(&mut self) -> &mut AggregateMetadata {
        &mut self.metadata
    }
}
