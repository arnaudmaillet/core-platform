use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use shared_kernel::domain::events::{AggregateRoot, AggregateMetadata};
use shared_kernel::domain::value_objects::{PushToken, RegionCode, Timezone, AccountId};
use shared_kernel::domain::entities::EntityMetadata;
use shared_kernel::errors::{DomainError, Result};
use crate::domain::builders::AccountSettingsBuilder;
use crate::domain::events::AccountEvent;

/// Cette struct représente exactement le contenu de la colonne JSONB 'settings'
#[derive(Serialize, Deserialize)]
pub struct SettingsBlob {
    pub privacy: PrivacySettings,
    pub notifications: NotificationSettings,
    pub appearance: AppearanceSettings,
}

#[derive(Debug, Clone)]
pub struct AccountSettings {
    pub account_id: AccountId,
    pub region_code: RegionCode,
    pub privacy: PrivacySettings,
    pub notifications: NotificationSettings,
    pub appearance: AppearanceSettings,
    pub timezone: Timezone,
    pub push_tokens: Vec<PushToken>,
    pub updated_at: DateTime<Utc>,
    pub metadata: AggregateMetadata,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PrivacySettings {
    pub profile_visible_to_public: bool,
    pub show_last_active: bool,
    pub allow_indexing: bool,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NotificationSettings {
    pub email_enabled: bool,
    pub push_enabled: bool,
    pub marketing_opt_in: bool,
    pub security_alerts_only: bool,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppearanceSettings {
    pub theme: ThemeMode,
    pub high_contrast: bool,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ThemeMode {
    Light,
    Dark,

    #[default]
    System,
}

impl AccountSettings {
    /// Point d'entrée pour le Builder
    pub fn builder(account_id: AccountId, region_code: RegionCode) -> AccountSettingsBuilder {
        AccountSettingsBuilder::new(account_id, region_code)
    }

    /// Met à jour la timezone avec un événement spécifique
    pub fn update_timezone(&mut self, new_tz: Timezone) -> Result<()> {
        if self.timezone == new_tz {
            return Ok(());
        }

        // Garde métier : Cohérence régionale (exemple Hyperscale)
        if self.region_code.as_str() == "eu" && new_tz.as_str().starts_with("America") {
            return Err(DomainError::Validation {
                field: "timezone",
                reason: "Inconsistent timezone for European region".into(),
            });
        }

        self.timezone = new_tz.clone();
        self.updated_at = Utc::now();

        self.add_event(Box::new(AccountEvent::TimezoneChanged {
            account_id: self.account_id.clone(),
            new_timezone: new_tz,
            occurred_at: self.updated_at,
        }));

        Ok(())
    }

    /// Ajoute un token avec événement spécifique et rotation FIFO
    pub fn add_push_token(&mut self, token: PushToken) -> Result<()> {
        if self.push_tokens.contains(&token) {
            return Ok(());
        }

        // Rotation FIFO (Max 10 tokens par utilisateur pour limiter la taille du champ)
        if self.push_tokens.len() >= 10 {
            self.push_tokens.remove(0);
        }

        self.push_tokens.push(token.clone());
        self.updated_at = Utc::now();

        self.add_event(Box::new(AccountEvent::PushTokenAdded {
            account_id: self.account_id.clone(),
            token,
            occurred_at: self.updated_at,
        }));

        Ok(())
    }

    /// Supprime un token (ex: au logout) avec événement spécifique
    pub fn remove_push_token(&mut self, token: &PushToken) -> Result<()> {
        let original_len = self.push_tokens.len();
        self.push_tokens.retain(|t| t != token);

        if self.push_tokens.len() == original_len {
            return Ok(());
        }

        self.updated_at = Utc::now();

        self.add_event(Box::new(AccountEvent::PushTokenRemoved {
            account_id: self.account_id.clone(),
            token: token.clone(),
            occurred_at: self.updated_at,
        }));

        Ok(())
    }

    /// Mise à jour globale
    pub fn update_preferences(
        &mut self,
        privacy: Option<PrivacySettings>,
        notifications: Option<NotificationSettings>,
        appearance: Option<AppearanceSettings>,
    ) -> Result<()> {
        let mut changed = false;

        if let Some(p) = privacy {
            if self.privacy != p {
                self.privacy = p;
                changed = true;
            }
        }
        if let Some(n) = notifications {
            if self.notifications != n {
                self.notifications = n;
                changed = true;
            }
        }
        if let Some(a) = appearance {
            if self.appearance != a {
                self.appearance = a;
                changed = true;
            }
        }

        if changed {
            self.updated_at = Utc::now();
            self.add_event(Box::new(AccountEvent::AccountSettingsUpdated {
                account_id: self.account_id.clone(),
                occurred_at: self.updated_at,
            }));
        }

        Ok(())
    }
}

impl EntityMetadata for AccountSettings {
    fn entity_name() -> &'static str { "AccountSettings" }

    fn map_constraint_to_field(constraint: &str) -> &'static str {
        match constraint {
            "account_settings_pkey" => "account_id",
            _ => "settings"
        }
    }
}


impl AggregateRoot for AccountSettings {
    fn id(&self) -> String {
        self.account_id.to_string()
    }
    fn metadata(&self) -> &AggregateMetadata {
        &self.metadata
    }
    fn metadata_mut(&mut self) -> &mut AggregateMetadata {
        &mut self.metadata
    }
}