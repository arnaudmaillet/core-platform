use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use shared_kernel::{
    domain::{
        events::{AggregateMetadata, AggregateRoot, DomainEvent},
        value_objects::{AccountId, AuditReason, Email, PhoneNumber, PushToken, RegionCode, SubId, Timezone, TrustContext},
    },
    errors::{DomainError, Result},
};

use crate::domain::{
    account::{
        builders::AccountBuilder,
        entities::{AccountGovernance, AccountIdentity, AccountSettings},
    },
    events::AccountEvent,
    preferences::models::{AppearancePreferences, NotificationPreferences, PrivacyPreferences},
    value_objects::{
        AccountRole, BirthDate, IpAddr, Locale,
        RegistrationIdentifier, TrustDelta,
    },
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    identity: AccountIdentity,
    governance: AccountGovernance,
    settings: AccountSettings,
    metadata: AggregateMetadata,
}

impl Account {
    pub fn builder(
        account_id: AccountId,
        region: RegionCode,
        identifier: RegistrationIdentifier,
    ) -> AccountBuilder {
        AccountBuilder::new(account_id, region, identifier)
    }

    pub(crate) fn restore(
        identity: AccountIdentity,
        governance: AccountGovernance,
        settings: AccountSettings,
        metadata: AggregateMetadata,
    ) -> Self {
        Self {
            identity,
            governance,
            settings,
            metadata,
        }
    }

    pub fn identity(&self) -> &AccountIdentity {
        &self.identity
    }
    pub fn governance(&self) -> &AccountGovernance {
        &self.governance
    }
    pub fn settings(&self) -> &AccountSettings {
        &self.settings
    }
    pub fn updated_at(&self) -> DateTime<Utc> {
        self.identity.aggregate_updated_at()
    }

    pub fn register(&mut self, region: RegionCode, ip_addr: IpAddr) -> Result<bool> {
        self.identity.apply_registration()?;
        self.governance.apply_ip_record(ip_addr.clone());

        self.apply_change();

        self.push_event(Box::new(AccountEvent::AccountRegistered {
            account_id: self.id_typed(),
            email: self.identity.email().cloned(),
            phone: self.identity.phone_number().cloned(),
            sub_id: self.identity.sub_id().cloned(),
            locale: self.identity.locale().clone(),
            region,
            ip_addr,
            occurred_at: self.updated_at(),
        }));

        Ok(true)
    }

    pub fn change_region(&mut self, new_region: RegionCode) -> Result<bool> {
        self.ensure_not_restricted()?;
        let old_region = self.identity.region_code().clone();

        self.track_change(
            |s| s.identity.apply_region_change(new_region.clone()),
            |s| {
                Box::new(AccountEvent::AccountRegionChanged {
                    account_id: s.id_typed(),
                    old_region,
                    new_region: new_region.clone(),
                    occurred_at: s.updated_at(),
                })
            },
        )
    }

    pub fn link_sub_identity(&mut self, new_id: SubId) -> Result<bool> {
        let current_id = self.identity.sub_id().cloned();

        // 1. Idempotence métier : si l'ID est déjà le même, on ne fait rien
        if current_id.as_ref() == Some(&new_id) {
            return Ok(false);
        }

        // 2. Garde de sécurité : on interdit d'écraser un lien existant
        // C'est ce check qui faisait paniquer ton test "forbidden"
        if current_id.is_some() {
            return Err(DomainError::Forbidden {
                reason: "Account is already linked to an sub provider".into(),
            });
        }

        // 3. Application du changement (Transition de None vers Some)
        self.track_change(
            |s| {
                s.identity.apply_sub_id_change(new_id.clone())?;
                Ok(true)
            },
            |s| {
                Box::new(AccountEvent::SubIdentityLinked {
                    account_id: s.id_typed(),
                    old_sub_id: current_id, // Sera None
                    new_sub_id: new_id.clone(),
                    occurred_at: s.updated_at(),
                })
            },
        )
    }

    pub fn change_birth_date(&mut self, new_date: BirthDate) -> Result<bool> {
        self.ensure_not_restricted()?;

        self.track_change(
            |s| s.identity.apply_birth_date_change(new_date),
            |s| {
                Box::new(AccountEvent::BirthDateChanged {
                    account_id: s.id_typed(),
                    occurred_at: Utc::now(),
                })
            },
        )
    }

    pub fn change_email(&mut self, new_email: Email) -> Result<bool> {
        self.ensure_not_restricted()?;
        let old_email = self.identity.email().cloned();

        self.track_change(
            |s| s.identity.apply_email_change(new_email.clone()),
            |s| {
                Box::new(AccountEvent::EmailChanged {
                    account_id: s.id_typed(),
                    old_email,
                    new_email: new_email.clone(),
                    occurred_at: s.updated_at(),
                })
            },
        )
    }

    pub fn change_phone(&mut self, new_phone: PhoneNumber) -> Result<bool> {
        self.ensure_not_restricted()?;
        let old_phone = self.identity.phone_number().cloned();

        self.track_change(
            |s| s.identity.apply_phone_change(new_phone.clone()),
            |s| {
                Box::new(AccountEvent::PhoneNumberChanged {
                    account_id: s.id_typed(),
                    old_phone_number: old_phone,
                    new_phone_number: new_phone.clone(),
                    occurred_at: s.updated_at(),
                })
            },
        )
    }

    pub fn ban(&mut self, reason: AuditReason) -> Result<bool> {
        let changed = self.track_change(
            |s| s.identity.apply_ban_state(),
            |s| {
                Box::new(AccountEvent::AccountBanned {
                    account_id: s.id_typed(),
                    reason: reason.clone().into(),
                    occurred_at: s.updated_at(),
                })
            },
        )?;

        if changed {
            self.governance.apply_trust_penalty(
                TrustDelta::PENALTY_BAN,
                TrustContext::AccountBanned,
                &reason,
            )?;
        }
        Ok(changed)
    }

    pub fn unban(&mut self, reason: AuditReason) -> Result<bool> {
        let changed = self.track_change(
            |s| s.identity.apply_unban_state(),
            |s| {
                Box::new(AccountEvent::AccountUnbanned {
                    account_id: s.id_typed(),
                    reason: reason.clone().into(),
                    occurred_at: s.updated_at(),
                })
            },
        )?;

        if changed {
            self.governance.apply_trust_reward(
                TrustDelta::REWARD_UNBAN,
                TrustContext::UnbanBonus,
                &reason,
            )?;
        }
        Ok(changed)
    }

    pub fn suspend(&mut self, reason: AuditReason) -> Result<bool> {
        self.track_change(
            |s| s.identity.apply_suspension_state(),
            |s| {
                Box::new(AccountEvent::AccountSuspended {
                    account_id: s.id_typed(),
                    reason: reason.into(),
                    occurred_at: s.updated_at(),
                })
            },
        )
    }

    pub fn unsuspend(&mut self, reason: AuditReason) -> Result<bool> {
        let changed = self.track_change(
            |s| s.identity.apply_unsuspend_state(),
            |s| {
                Box::new(AccountEvent::AccountUnsuspended {
                    account_id: s.id_typed(),
                    reason: reason.clone().into(),
                    occurred_at: s.updated_at(),
                })
            },
        )?;

        if changed {
            self.governance.apply_trust_reward(
                TrustDelta::REWARD_UNSUSPEND,
                TrustContext::SuspensionLifted,
                &reason,
            )?;
        }
        Ok(changed)
    }

    pub fn activate(&mut self) -> Result<bool> {
        if self.identity.is_banned() {
            return Err(DomainError::Forbidden {
                reason: "Banned accounts must be unbanned, not just activated".into(),
            });
        }
        if self.identity.is_suspended() {
            return Err(DomainError::Forbidden {
                reason: "Suspended accounts must be unsuspend, not just activated".into(),
            });
        }

        self.track_change(
            |s| s.identity.apply_active_state(),
            |s| {
                Box::new(AccountEvent::AccountActivated {
                    account_id: s.id_typed(),
                    reason: "User initiated activation".into(),
                    occurred_at: s.updated_at(),
                })
            },
        )
    }

    pub fn deactivate(&mut self, reason: Option<AuditReason>) -> Result<bool> {
        let final_reason = reason
            .map(|r| r.to_string())
            .unwrap_or_else(|| "User initiated deactivation".to_string());

        self.track_change(
            |s| s.identity.apply_deactivation_state(),
            |s| {
                Box::new(AccountEvent::AccountDeactivated {
                    account_id: s.id_typed(),
                    reason: final_reason,
                    occurred_at: s.updated_at(),
                })
            },
        )
    }

    pub fn reward_trust(&mut self, amount: TrustDelta, reason: AuditReason) -> Result<bool> {
        self.track_change(
            |s| {
                s.governance
                    .apply_trust_reward(amount, TrustContext::ManualAdjustment, &reason)
            },
            |s| {
                Box::new(AccountEvent::TrustScoreAdjusted {
                    id: uuid::Uuid::new_v4(),
                    account_id: s.id_typed(),
                    delta: amount,
                    new_score: s.governance.trust_score(),
                    reason: reason.clone().into(),
                    occurred_at: s.updated_at(),
                })
            },
        )
    }

    pub fn shadowban(&mut self, reason: AuditReason) -> Result<bool> {
        self.track_change(
            |s| s.governance.apply_shadowban(&reason),
            |s| {
                Box::new(AccountEvent::ShadowbanUpdated {
                    account_id: s.id_typed(),
                    is_shadowbanned: true,
                    reason: reason.clone().into(),
                    occurred_at: s.updated_at(),
                })
            },
        )
    }

    pub fn penalize_trust(&mut self, amount: TrustDelta, reason: AuditReason) -> Result<bool> {
        let mut extra_event: Option<Box<dyn DomainEvent>> = None;
        let auto_reason = AuditReason::system("Trust score critical threshold reached");

        let changed = self.track_change(
            |s| {
                let score_changed = s.governance.apply_trust_penalty(
                    amount,
                    TrustContext::ManualAdjustment,
                    &reason,
                )?;

                if s.governance.trust_score().is_critical() && !s.governance.is_shadowbanned() {
                    s.governance.apply_shadowban(&auto_reason)?;

                    extra_event = Some(Box::new(AccountEvent::ShadowbanUpdated {
                        account_id: s.id_typed(),
                        is_shadowbanned: true,
                        reason: auto_reason.into(),
                        occurred_at: s.updated_at(),
                    }));
                    return Ok(true);
                }
                Ok(score_changed)
            },
            |s| {
                Box::new(AccountEvent::TrustScoreAdjusted {
                    id: uuid::Uuid::new_v4(),
                    account_id: s.id_typed(),
                    delta: -amount,
                    new_score: s.governance.trust_score(),
                    reason: reason.clone().into(),
                    occurred_at: s.updated_at(),
                })
            },
        )?;

        if let Some(event) = extra_event {
            self.push_event(event);
        }

        Ok(changed)
    }

    pub fn lift_shadowban(&mut self, reason: AuditReason) -> Result<bool> {
        self.track_change(
            |s| s.governance.apply_lift_shadowban(&reason),
            |s| {
                Box::new(AccountEvent::ShadowbanUpdated {
                    account_id: s.id_typed(),
                    is_shadowbanned: false,
                    reason: reason.clone().into(),
                    occurred_at: s.updated_at(),
                })
            },
        )
    }

    pub fn change_role(&mut self, new_role: AccountRole, reason: AuditReason) -> Result<bool> {
        self.ensure_not_restricted()?;
        let old_role = self.governance.role();

        self.track_change(
            |s| s.governance.apply_role_change(new_role, &reason),
            |s| {
                Box::new(AccountEvent::AccountRoleChanged {
                    account_id: s.id_typed(),
                    old_role,
                    new_role: new_role.clone(),
                    reason: reason.clone().into(),
                    occurred_at: s.updated_at(),
                })
            },
        )
    }

    pub fn update_timezone(&mut self, new_tz: Timezone) -> Result<bool> {
        self.ensure_not_restricted()?;
        let region = self.identity.region_code().clone();

        self.track_change(
            |s| s.settings.apply_timezone_update(new_tz.clone(), &region),
            |s| {
                Box::new(AccountEvent::TimezoneUpdated {
                    account_id: s.id_typed(),
                    new_timezone: new_tz.clone(),
                    occurred_at: s.updated_at(),
                })
            },
        )
    }

    pub fn update_locale(&mut self, new_locale: Locale) -> Result<bool> {
        self.ensure_not_restricted()?;

        self.track_change(
            |s| s.identity.apply_locale_change(new_locale.clone()),
            |s| {
                Box::new(AccountEvent::LocaleUpdated {
                    account_id: s.id_typed(),
                    new_locale: new_locale.clone(),
                    occurred_at: s.updated_at(),
                })
            },
        )
    }

    pub fn add_push_token(&mut self, token: PushToken) -> Result<bool> {
        self.ensure_not_restricted()?;
        self.track_change(
            |s| Ok(s.settings.apply_push_token_add(token.clone())),
            |s| {
                Box::new(AccountEvent::PushTokenAdded {
                    account_id: s.id_typed(),
                    token: token.clone(),
                    occurred_at: s.updated_at(),
                })
            },
        )
    }

    pub fn remove_push_token(&mut self, token: PushToken) -> Result<bool> {
        self.ensure_not_restricted()?;
        self.track_change(
            |s| Ok(s.settings.apply_push_token_remove(&token)),
            |s| {
                Box::new(AccountEvent::PushTokenRemoved {
                    account_id: s.id_typed(),
                    token: token.clone(),
                    occurred_at: s.updated_at(),
                })
            },
        )
    }

    pub fn update_notifications_preferences(
        &mut self,
        new_prefs: NotificationPreferences,
    ) -> Result<bool> {
        self.ensure_not_restricted()?;
        self.track_change(
            |s| Ok(s.settings.apply_notifications_update(new_prefs.clone())),
            |s| {
                Box::new(AccountEvent::NotificationsPreferencesUpdated {
                    account_id: s.id_typed(),
                    new_preferences: new_prefs.clone(),
                    occurred_at: s.updated_at(),
                })
            },
        )
    }

    pub fn update_appearance_preferences(
        &mut self,
        new_prefs: AppearancePreferences,
    ) -> Result<bool> {
        self.ensure_not_restricted()?;
        self.track_change(
            |s| Ok(s.settings.apply_appearance_update(new_prefs.clone())),
            |s| {
                Box::new(AccountEvent::AppearancePreferencesUpdated {
                    account_id: s.id_typed(),
                    new_preferences: new_prefs.clone(),
                    occurred_at: s.updated_at(),
                })
            },
        )
    }

    pub fn update_privacy_preferences(&mut self, new_prefs: PrivacyPreferences) -> Result<bool> {
        self.ensure_not_restricted()?;
        self.track_change(
            |s| Ok(s.settings.apply_privacy_update(new_prefs.clone())),
            |s| {
                Box::new(AccountEvent::PrivacyPreferencesUpdated {
                    account_id: s.id_typed(),
                    new_preferences: new_prefs.clone(),
                    occurred_at: s.updated_at(),
                })
            },
        )
    }

    pub fn record_activity(&mut self) -> Result<bool> {
        let changed = self.identity.apply_activity_record()?;
        if changed {
            self.apply_change();
        }

        Ok(changed)
    }

    fn ensure_not_restricted(&self) -> Result<()> {
        if self.identity.is_blocked() {
            return Err(DomainError::Forbidden {
                reason:
                    "Operation forbidden: account is restricted (banned, suspended or deactivated)"
                        .into(),
            });
        }
        Ok(())
    }

    // ==========================================
    // INFRASTRUCTURE & MAPPING
    // ==========================================

    fn id_typed(&self) -> AccountId {
        self.identity.account_id().clone()
    }

    fn apply_change(&mut self) {
        self.record_change();
    }

    // --- HELPER DE COORDINATION ---
    fn track_change<F>(
        &mut self,
        action: F,
        event_factory: impl FnOnce(&Self) -> Box<dyn DomainEvent>,
    ) -> Result<bool>
    where
        F: FnOnce(&mut Self) -> Result<bool>,
    {
        if action(self)? {
            self.apply_change(); // Version +1
            let event = event_factory(self);
            self.push_event(event);
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

impl AggregateRoot for Account {
    fn id(&self) -> String {
        self.identity.account_id().to_string()
    }
    fn metadata(&self) -> &AggregateMetadata {
        &self.metadata
    }
    fn metadata_mut(&mut self) -> &mut AggregateMetadata {
        &mut self.metadata
    }
}
