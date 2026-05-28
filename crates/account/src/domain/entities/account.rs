use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use shared_kernel::{
    core::{AggregateMetadata, AggregateRoot, Error, Result, Versioned},
    geo::Timezone,
    messaging::{Event, EventEmitter, OperationTracker},
    security::{PushToken, TrustContext},
    types::{AccountId, AuditReason, Email, PhoneNumber, Region, SubId},
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
    metadata: AggregateMetadata,
}

impl Versioned for Account {
    fn version(&self) -> u64 {
        self.metadata.version()
    }
    fn updated_at(&self) -> DateTime<Utc> {
        self.metadata.updated_at()
    }
    fn record_change(&mut self) {
        self.metadata.record_change();
    }
}

impl EventEmitter for Account {
    fn push_event(&mut self, event: Box<dyn Event>) {
        self.metadata.push_event(event);
    }
    fn pull_events(&mut self) -> Vec<Box<dyn Event>> {
        self.metadata.pull_events()
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

impl Account {
    pub fn builder(account_id: AccountId, identifier: RegistrationIdentifier) -> AccountBuilder {
        AccountBuilder::new(account_id, identifier)
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
    pub fn updated_at(&self) -> DateTime<Utc> {
        self.identity.aggregate_updated_at()
    }

    fn id_typed(&self) -> AccountId {
        self.identity.account_id()
    }

    pub fn record_activity(&mut self) -> Result<bool> {
        let changed = self.identity.apply_activity_record()?;

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
                    account_id: s.id_typed(),
                    email: s.identity.email().cloned(),
                    phone: s.identity.phone_number().cloned(),
                    sub_id: s.identity.sub_id().cloned(),
                    locale: s.identity.locale().clone(),
                    ip_addr,
                    occurred_at: s.updated_at(),
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
    //                 account_id: s.id_typed(),
    //                 old_region,
    //                 new_region: new_region,
    //                 occurred_at: s.updated_at(),
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
                    account_id: s.id_typed(),
                    old_sub_id: current_id,
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
            // 💡 ALIGNEMENT : Utilisation de TrustAmount::PENALTY_BAN
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
                    account_id: s.id_typed(),
                    reason: reason.clone().into(),
                    occurred_at: s.updated_at(),
                })
            },
        )?;

        if changed {
            // 💡 ALIGNEMENT : Utilisation de TrustAmount::REWARD_UNBAN
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

    pub fn reward_trust(&mut self, amount: TrustAmount, reason: AuditReason) -> Result<bool> {
        self.track_change(
            |s| {
                s.governance
                    .apply_trust_reward(amount, TrustContext::ManualAdjustment, &reason)
            },
            |s| {
                Box::new(AccountEvent::TrustScoreRewarded {
                    id: uuid::Uuid::new_v4(),
                    account_id: s.id_typed(),
                    amount,
                    new_score: s.governance.trust_score(),
                    reason: reason.clone().into(),
                    occurred_at: s.updated_at(),
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
                Box::new(AccountEvent::TrustScorePenalized {
                    id: uuid::Uuid::new_v4(),
                    account_id: s.id_typed(),
                    amount,
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

    pub fn change_beta_tier(&mut self, new_tier: BetaTier) -> Result<bool> {
        self.ensure_not_restricted()?;
        let old_tier = self.governance.beta_tier();

        self.track_change(
            |s| s.governance.apply_beta_tier_change(new_tier),
            |s| {
                Box::new(AccountEvent::BetaTierChanged {
                    account_id: s.id_typed(),
                    old_tier,
                    new_tier: new_tier.clone(),
                    occurred_at: s.updated_at(),
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

    fn ensure_not_restricted(&self) -> Result<()> {
        if self.identity.is_blocked() {
            return Err(Error::forbidden(
                "Operation forbidden: account is restricted (banned, suspended or deactivated)",
            ));
        }
        Ok(())
    }
}
