use crate::domain::builders::AccountSettingsBuilder;
use crate::domain::events::AccountEvent;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use shared_kernel::domain::Identifier;
use shared_kernel::domain::entities::EntityMetadata;
use shared_kernel::domain::events::{AggregateMetadata, AggregateRoot};
use shared_kernel::domain::value_objects::{AccountId, PushToken, RegionCode, Timezone};
use shared_kernel::errors::{DomainError, Result};

/// Cette struct représente exactement le contenu de la colonne JSONB 'settings'
#[derive(Serialize, Deserialize)]
pub struct SettingsBlob {
    pub privacy: PrivacySettings,
    pub notifications: NotificationSettings,
    pub appearance: AppearanceSettings,
}

#[derive(Debug, Clone)]
pub struct AccountSettings {
    account_id: AccountId,
    region_code: RegionCode,
    privacy: PrivacySettings,
    notifications: NotificationSettings,
    appearance: AppearanceSettings,
    timezone: Timezone,
    push_tokens: Vec<PushToken>,
    updated_at: DateTime<Utc>,
    metadata: AggregateMetadata,
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
    /// Utilisé par le Builder et le Repository
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn restore(
        account_id: AccountId,
        region_code: RegionCode,
        privacy: PrivacySettings,
        notifications: NotificationSettings,
        appearance: AppearanceSettings,
        timezone: Timezone,
        push_tokens: Vec<PushToken>,
        updated_at: DateTime<Utc>,
        metadata: AggregateMetadata,
    ) -> Self {
        Self {
            account_id,
            region_code,
            privacy,
            notifications,
            appearance,
            timezone,
            push_tokens,
            updated_at,
            metadata,
        }
    }

    // --- GETTERS PUBLICS ---

    pub fn account_id(&self) -> &AccountId { &self.account_id }
    pub fn region_code(&self) -> &RegionCode { &self.region_code }
    pub fn privacy(&self) -> &PrivacySettings { &self.privacy }
    pub fn notifications(&self) -> &NotificationSettings { &self.notifications }
    pub fn appearance(&self) -> &AppearanceSettings { &self.appearance }
    pub fn timezone(&self) -> &Timezone { &self.timezone }
    pub fn push_tokens(&self) -> &[PushToken] { &self.push_tokens }
    pub fn updated_at(&self) -> DateTime<Utc> { self.updated_at }


    /// Change la région des paramètres (nécessaire pour la cohérence du sharding)
    pub fn change_region(&mut self, new_region: RegionCode) -> Result<bool> {
        if self.region_code == new_region {
            return Ok(false);
        }
        self.region_code = new_region;
        self.apply_change();

        Ok(true)
    }

    /// Met à jour la timezone avec un événement spécifique
    pub fn update_timezone(&mut self, region: &RegionCode, new_tz: Timezone) -> Result<bool> {
        self.ensure_region_match(region)?;
        if self.timezone == new_tz {
            return Ok(false);
        }

        // Garde métier : Cohérence régionale (exemple Hyperscale)
        if self.region_code.as_str() == "eu" && new_tz.as_str().starts_with("America") {
            return Err(DomainError::Validation {
                field: "timezone",
                reason: "Inconsistent timezone for European region".into(),
            });
        }

        self.timezone = new_tz.clone();
        self.apply_change();

        self.add_event(Box::new(AccountEvent::TimezoneChanged {
            account_id: self.account_id.clone(),
            region: self.region_code.clone(),
            new_timezone: new_tz,
            occurred_at: self.updated_at,
        }));

        Ok(true)
    }

    /// Ajoute un token avec événement spécifique et rotation FIFO
    pub fn add_push_token(&mut self, region: &RegionCode, token: PushToken) -> Result<bool> {
        self.ensure_region_match(region)?;
        if self.push_tokens.contains(&token) {
            return Ok(false);
        }

        // Rotation FIFO (Max 10 tokens par utilisateur pour limiter la taille du champ)
        if self.push_tokens.len() >= 10 {
            self.push_tokens.remove(0);
        }

        self.push_tokens.push(token.clone());
        self.apply_change();

        self.add_event(Box::new(AccountEvent::PushTokenAdded {
            account_id: self.account_id.clone(),
            region: self.region_code.clone(),
            token,
            occurred_at: self.updated_at,
        }));

        Ok(true)
    }

    /// Supprime un token (ex: au logout) avec événement spécifique
    pub fn remove_push_token(&mut self, region: &RegionCode, token: &PushToken) -> Result<bool> {
        self.ensure_region_match(region)?;
        let original_len = self.push_tokens.len();
        self.push_tokens.retain(|t| t != token);

        if self.push_tokens.len() == original_len {
            return Ok(false);
        }

        self.apply_change();

        self.add_event(Box::new(AccountEvent::PushTokenRemoved {
            account_id: self.account_id.clone(),
            region: self.region_code.clone(),
            token: token.clone(),
            occurred_at: self.updated_at,
        }));

        Ok(true)
    }

    /// Mise à jour globale
    pub fn update_preferences(
        &mut self,
        region: &RegionCode,
        privacy: Option<PrivacySettings>,
        notifications: Option<NotificationSettings>,
        appearance: Option<AppearanceSettings>,
    ) -> Result<bool> {
        self.ensure_region_match(region)?;

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
            self.apply_change();
            self.add_event(Box::new(AccountEvent::AccountSettingsUpdated {
                account_id: self.account_id.clone(),
                region: self.region_code.clone(),
                occurred_at: self.updated_at,
            }));
        }

        Ok(changed)
    }


    // --- LOGIQUE DE VERSIONING ---

    fn apply_change(&mut self) {
        self.increment_version(); // Méthode de AggregateRoot
        self.updated_at = Utc::now();
    }

    fn ensure_region_match(&self, region: &RegionCode) -> Result<()> {
        if &self.region_code != region {
            return Err(DomainError::Forbidden {
                reason: "Cross-region operation detected".into(),
            });
        }
        Ok(())
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
