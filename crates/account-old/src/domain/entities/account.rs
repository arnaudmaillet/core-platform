use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use shared_kernel::{
    core::{Entity, Error, LifecycleTracker, ManagedEntity, Result, Versioned},
    geo::Timezone,
    messaging::{Event, EventEmitter, OperationTracker},
    security::{PushToken, TrustContext},
    types::{AccountId, AuditReason, Email, Phone, SubId},
};

use crate::{
    domain::{
        entities::{AccountBuilder, AccountGovernance, AccountIdentity, AccountSettings},
        events::AccountEvent,
        types::{
            AccountRole, AppearancePreferences, BirthDate, IpAddr, Locale, NotificationPreferences,
            PrivacyPreferences, RegistrationIdentifier, TrustAmount,
        },
    },
    types::BetaTier,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    identity: AccountIdentity,
    governance: AccountGovernance,
    settings: AccountSettings,
    lifecycle: LifecycleTracker,
    version: u64,
}

impl Versioned for Account {
    fn version(&self) -> u64 {
        self.version
    }
    fn updated_at(&self) -> DateTime<Utc> {
        self.lifecycle.updated_at()
    }
    fn record_change(&mut self) {
        self.version += 1;
        self.lifecycle.record_change();
    }
}

impl EventEmitter for Account {
    fn push_event(&mut self, event: Box<dyn Event>) {
        self.lifecycle.push_event(event);
    }
    fn pull_events(&mut self) -> Vec<Box<dyn Event>> {
        self.lifecycle.pull_events()
    }
}

impl ManagedEntity for Account {
    fn lifecycle(&self) -> &LifecycleTracker {
        &self.lifecycle
    }

    fn lifecycle_mut(&mut self) -> &mut LifecycleTracker {
        &mut self.lifecycle
    }
}

impl Entity for Account {
    type Id = AccountId;

    fn entity_name() -> &'static str {
        "Account"
    }

    fn map_constraint_to_field(constraint: &str) -> &'static str {
        match constraint {
            "accounts_pkey" => "account_id",
            "accounts_email_key" => "email",
            _ => "internal_security",
        }
    }

    fn id(&self) -> &Self::Id {
        &self.identity.account_id_as_ref()
    }

    fn updated_at(&self) -> DateTime<Utc> {
        self.lifecycle.updated_at()
    }
}

impl Account {
    pub fn builder(account_id: AccountId, identifier: RegistrationIdentifier) -> AccountBuilder {
        AccountBuilder::new(account_id, identifier)
    }

    pub(crate) fn restore(
        identity: AccountIdentity,
        governance: AccountGovernance,
        settings: AccountSettings,
        version: u64,
        lifecycle: LifecycleTracker,
    ) -> Self {
        Self {
            identity,
            governance,
            settings,
            version,
            lifecycle,
        }
    }

    pub fn account_id(&self) -> AccountId {
        self.identity.account_id()
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

    fn track_change<F, E>(&mut self, action: F, event_factory: E) -> Result<bool>
    where
        F: FnOnce(&mut Self) -> Result<bool>,
        E: FnOnce(&Self) -> Box<dyn Event>,
    {
        let changed = OperationTracker::track_change(self, action, event_factory)?;

        if changed {
            self.record_change();
        }

        Ok(changed)
    }

    pub fn register(&mut self, ip_addr: IpAddr) -> Result<bool> {
        self.track_change(
            |s| {
                s.identity.apply_registration()?;
                s.governance.apply_ip_record(ip_addr.clone());
                Ok(true)
            },
            |s| {
                Box::new(AccountEvent::AccountRegistered {
                    account_id: s.account_id(),
                    email: s.identity.email().cloned(),
                    phone: s.identity.phone().cloned(),
                    sub_id: s.identity.sub_id().cloned(),
                    locale: s.identity.locale().clone(),
                    ip_addr,
                    occurred_at: s.lifecycle().updated_at(),
                })
            },
        )
    }

    // pub fn change_region(&mut self, new_region: Region) -> Result<bool> {
    //     self.ensure_not_restricted()?;
    //     let old_region = self.identity.region().clone();

    //     self.track_change(
    //         |_| {
    //             if old_region == new_region {
    //                 Ok(false)
    //             } else {
    //                 Ok(true)
    //             }
    //         },
    //         |s| {
    //             Box::new(AccountEvent::AccountRegionChanged {
    //                 account_id: s.account_id(),
    //                 old_region,
    //                 new_region: new_region,
    //                 occurred_at: s.lifecycle().updated_at(),
    //             })
    //         },
    //     )
    // }

    pub fn link_sub_identity(&mut self, new_id: SubId) -> Result<bool> {
        let current_id = self.identity.sub_id().cloned();

        if current_id.as_ref() == Some(&new_id) {
            return Ok(false);
        }

        if current_id.is_some() {
            return Err(Error::forbidden(
                "Account is already linked to an sub provider",
            ));
        }

        self.track_change(
            |s| {
                s.identity.apply_sub_id_change(new_id.clone())?;
                Ok(true)
            },
            |s| {
                Box::new(AccountEvent::SubIdentityLinked {
                    account_id: s.account_id(),
                    old_sub_id: current_id,
                    new_sub_id: new_id.clone(),
                    occurred_at: s.lifecycle().updated_at(),
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
                    account_id: s.account_id(),
                    old_birth_date: s.identity.birth_date().cloned(),
                    new_birth_date: new_date,
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
                    account_id: s.account_id(),
                    old_email,
                    new_email: new_email.clone(),
                    occurred_at: s.lifecycle().updated_at(),
                })
            },
        )
    }

    pub fn change_phone(&mut self, new_phone: Phone) -> Result<bool> {
        self.ensure_not_restricted()?;
        let old_phone = self.identity.phone().cloned();

        self.track_change(
            |s| s.identity.apply_phone_change(new_phone.clone()),
            |s| {
                Box::new(AccountEvent::PhoneChanged {
                    account_id: s.account_id(),
                    old_phone,
                    new_phone: new_phone.clone(),
                    occurred_at: s.lifecycle().updated_at(),
                })
            },
        )
    }

    pub fn verify_email(&mut self, verified_at: DateTime<Utc>) -> Result<bool> {
        self.ensure_not_restricted()?;

        let target_email =
            self.identity.email().cloned().ok_or_else(|| {
                Error::validation("email", "Cannot verify an empty email address")
            })?;

        self.track_change(
            |s| s.identity.apply_email_verification(verified_at),
            |s| {
                Box::new(AccountEvent::EmailVerified {
                    account_id: s.account_id(),
                    email: target_email,
                    occurred_at: s.lifecycle().updated_at(),
                })
            },
        )
    }

    // 💡 NOUVEAU : Action souveraine de validation de numéro de téléphone
    pub fn verify_phone(&mut self, verified_at: DateTime<Utc>) -> Result<bool> {
        self.ensure_not_restricted()?;

        let target_phone = self
            .identity
            .phone()
            .cloned()
            .ok_or_else(|| Error::validation("phone", "Cannot verify an empty phone number"))?;

        self.track_change(
            |s| s.identity.apply_phone_verification(verified_at),
            |s| {
                Box::new(AccountEvent::PhoneVerified {
                    account_id: s.account_id(),
                    phone: target_phone,
                    occurred_at: s.lifecycle().updated_at(),
                })
            },
        )
    }

    pub fn ban(&mut self, reason: AuditReason) -> Result<bool> {
        let changed = self.track_change(
            |s| s.identity.apply_ban_state(),
            |s| {
                Box::new(AccountEvent::AccountBanned {
                    account_id: s.account_id(),
                    reason: reason.clone().into(),
                    occurred_at: s.lifecycle().updated_at(),
                })
            },
        )?;

        if changed {
            self.governance.apply_trust_penalty(
                TrustAmount::PENALTY_BAN,
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
                    account_id: s.account_id(),
                    reason: reason.clone().into(),
                    occurred_at: s.lifecycle().updated_at(),
                })
            },
        )?;

        if changed {
            self.governance.apply_trust_reward(
                TrustAmount::REWARD_UNBAN,
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
                    account_id: s.account_id(),
                    reason: reason.into(),
                    occurred_at: s.lifecycle().updated_at(),
                })
            },
        )
    }

    pub fn unsuspend(&mut self, reason: AuditReason) -> Result<bool> {
        let changed = self.track_change(
            |s| s.identity.apply_unsuspend_state(),
            |s| {
                Box::new(AccountEvent::AccountUnsuspended {
                    account_id: s.account_id(),
                    reason: reason.clone().into(),
                    occurred_at: s.lifecycle().updated_at(),
                })
            },
        )?;

        if changed {
            // 💡 ALIGNEMENT : Utilisation de TrustAmount::REWARD_UNSUSPEND
            self.governance.apply_trust_reward(
                TrustAmount::REWARD_UNSUSPEND,
                TrustContext::SuspensionLifted,
                &reason,
            )?;
        }
        Ok(changed)
    }

    pub fn activate(&mut self) -> Result<bool> {
        if self.identity.is_banned() {
            return Err(Error::forbidden(
                "Banned accounts must be unbanned, not just activated",
            ));
        }
        if self.identity.is_suspended() {
            return Err(Error::forbidden(
                "Suspended accounts must be unsuspend, not just activated",
            ));
        }

        self.track_change(
            |s| s.identity.apply_active_state(),
            |s| {
                Box::new(AccountEvent::AccountActivated {
                    account_id: s.account_id(),
                    reason: "User initiated activation".into(),
                    occurred_at: s.lifecycle().updated_at(),
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
                    account_id: s.account_id(),
                    reason: final_reason,
                    occurred_at: s.lifecycle().updated_at(),
                })
            },
        )
    }

    pub fn reward_trust(&mut self, amount: TrustAmount, reason: AuditReason) -> Result<bool> {
        self.track_change(
            |s| {
                s.governance
                    .apply_trust_reward(amount, TrustContext::ManualAdjustment, &reason)
            },
            |s| {
                Box::new(AccountEvent::TrustScoreRewarded {
                    id: uuid::Uuid::new_v4(),
                    account_id: s.account_id(),
                    amount,
                    new_score: s.governance.trust_score(),
                    reason: reason.clone().into(),
                    occurred_at: s.lifecycle().updated_at(),
                })
            },
        )
    }

    pub fn penalize_trust(&mut self, amount: TrustAmount, reason: AuditReason) -> Result<bool> {
        let mut extra_event: Option<Box<dyn Event>> = None;
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
                        account_id: s.account_id(),
                        is_shadowbanned: true,
                        reason: auto_reason.into(),
                        occurred_at: s.lifecycle().updated_at(),
                    }));
                    return Ok(true);
                }
                Ok(score_changed)
            },
            |s| {
                Box::new(AccountEvent::TrustScorePenalized {
                    id: uuid::Uuid::new_v4(),
                    account_id: s.account_id(),
                    amount,
                    new_score: s.governance.trust_score(),
                    reason: reason.clone().into(),
                    occurred_at: s.lifecycle().updated_at(),
                })
            },
        )?;

        if let Some(event) = extra_event {
            self.push_event(event);
        }

        Ok(changed)
    }

    pub fn shadowban(&mut self, reason: AuditReason) -> Result<bool> {
        self.track_change(
            |s| s.governance.apply_shadowban(&reason),
            |s| {
                Box::new(AccountEvent::ShadowbanUpdated {
                    account_id: s.account_id(),
                    is_shadowbanned: true,
                    reason: reason.clone().into(),
                    occurred_at: s.lifecycle().updated_at(),
                })
            },
        )
    }

    pub fn lift_shadowban(&mut self, reason: AuditReason) -> Result<bool> {
        self.track_change(
            |s| s.governance.apply_lift_shadowban(&reason),
            |s| {
                Box::new(AccountEvent::ShadowbanUpdated {
                    account_id: s.account_id(),
                    is_shadowbanned: false,
                    reason: reason.clone().into(),
                    occurred_at: s.lifecycle().updated_at(),
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
                    account_id: s.account_id(),
                    old_role,
                    new_role: new_role.clone(),
                    reason: reason.clone().into(),
                    occurred_at: s.lifecycle().updated_at(),
                })
            },
        )
    }

    pub fn change_beta_tier(&mut self, new_tier: BetaTier) -> Result<bool> {
        self.ensure_not_restricted()?;
        let old_tier = self.governance.beta_tier();

        self.track_change(
            |s| s.governance.apply_beta_tier_change(new_tier),
            |s| {
                Box::new(AccountEvent::BetaTierChanged {
                    account_id: s.account_id(),
                    old_tier,
                    new_tier: new_tier.clone(),
                    occurred_at: s.lifecycle().updated_at(),
                })
            },
        )
    }

    pub fn update_timezone(&mut self, new_tz: Timezone) -> Result<bool> {
        self.ensure_not_restricted()?;

        self.track_change(
            |s| s.settings.apply_timezone_update(new_tz.clone()),
            |s| {
                Box::new(AccountEvent::TimezoneUpdated {
                    account_id: s.account_id(),
                    new_timezone: new_tz.clone(),
                    occurred_at: s.lifecycle().updated_at(),
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
                    account_id: s.account_id(),
                    new_locale: new_locale.clone(),
                    occurred_at: s.lifecycle().updated_at(),
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
                    account_id: s.account_id(),
                    token: token.clone(),
                    occurred_at: s.lifecycle().updated_at(),
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
                    account_id: s.account_id(),
                    token: token.clone(),
                    occurred_at: s.lifecycle().updated_at(),
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
                    account_id: s.account_id(),
                    new_preferences: new_prefs.clone(),
                    occurred_at: s.lifecycle().updated_at(),
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
                    account_id: s.account_id(),
                    new_preferences: new_prefs.clone(),
                    occurred_at: s.lifecycle().updated_at(),
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
                    account_id: s.account_id(),
                    new_preferences: new_prefs.clone(),
                    occurred_at: s.lifecycle().updated_at(),
                })
            },
        )
    }

    fn ensure_not_restricted(&self) -> Result<()> {
        if self.identity.is_blocked() {
            return Err(Error::forbidden(
                "Operation forbidden: account is restricted (banned, suspended or deactivated)",
            ));
        }
        Ok(())
    }
}
