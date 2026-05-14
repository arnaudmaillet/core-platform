// crates/account/src/domain/entities/identity.rs

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use shared_kernel::{
    core::{Entity, Result},
    types::{AccountId, Email, PhoneNumber, RegionCode, SubId},
};

use crate::domain::{
    entities::AccountIdentityBuilder,
    types::{AccountState, BirthDate, Locale},
};

/// Entité Identity (Interne à l'Agrégat Account)
///
/// Gère les données brutes d'identification et les transitions d'état local.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountIdentity {
    account_id: AccountId,
    sub_id: Option<SubId>,
    email: Option<Email>,
    phone_number: Option<PhoneNumber>,
    state: AccountState,
    birth_date: Option<BirthDate>,
    locale: Locale,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    aggregate_updated_at: DateTime<Utc>,
    last_active_at: Option<DateTime<Utc>>,
}

impl AccountIdentity {
    pub fn builder(account_id: AccountId) -> AccountIdentityBuilder {
        AccountIdentityBuilder::new(account_id)
    }

    /// Utilisé par le Builder ou le Repository pour restaurer l'état.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn restore(
        account_id: AccountId,
        sub_id: Option<SubId>,
        email: Option<Email>,
        phone_number: Option<PhoneNumber>,
        state: AccountState,
        birth_date: Option<BirthDate>,
        locale: Locale,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
        aggregate_updated_at: DateTime<Utc>,
        last_active_at: Option<DateTime<Utc>>,
    ) -> Self {
        Self {
            account_id,
            sub_id,
            email,
            phone_number,
            state,
            birth_date,
            locale,
            created_at,
            updated_at,
            aggregate_updated_at,
            last_active_at,
        }
    }

    // --- GETTERS ---

    pub fn account_id(&self) -> &AccountId {
        &self.account_id
    }
    pub fn region_code(&self) -> &RegionCode {
        self.account_id.region()
    }
    pub fn sub_id(&self) -> Option<&SubId> {
        self.sub_id.as_ref()
    }
    pub fn email(&self) -> Option<&Email> {
        self.email.as_ref()
    }
    pub fn phone_number(&self) -> Option<&PhoneNumber> {
        self.phone_number.as_ref()
    }
    pub fn state(&self) -> &AccountState {
        &self.state
    }
    pub fn birth_date(&self) -> Option<&BirthDate> {
        self.birth_date.as_ref()
    }
    pub fn locale(&self) -> &Locale {
        &self.locale
    }
    pub fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }
    pub fn aggregate_updated_at(&self) -> DateTime<Utc> {
        self.aggregate_updated_at
    }
    pub fn last_active_at(&self) -> Option<DateTime<Utc>> {
        self.last_active_at
    }

    // ==========================================
    // MUTATIONS INTERNES (pub(crate))
    // ==========================================

    pub(crate) fn apply_region_change(&mut self, new_region: RegionCode) -> Result<bool> {
        if self.region_code() == &new_region {
            return Ok(false);
        }

        self.account_id = AccountId::new(self.account_id.uuid(), new_region);
        Ok(true)
    }

    pub(crate) fn apply_sub_id_change(&mut self, new_sub_id: SubId) -> Result<bool> {
        if self.sub_id.as_ref() == Some(&new_sub_id) {
            return Ok(false);
        }
        self.sub_id = Some(new_sub_id);
        Ok(true)
    }

    pub(crate) fn apply_email_change(&mut self, new_email: Email) -> Result<bool> {
        if self.email.as_ref() == Some(&new_email) {
            return Ok(false);
        }
        self.email = Some(new_email);
        Ok(true)
    }

    pub(crate) fn apply_phone_change(&mut self, new_phone: PhoneNumber) -> Result<bool> {
        if self.phone_number.as_ref() == Some(&new_phone) {
            return Ok(false);
        }
        self.phone_number = Some(new_phone);
        Ok(true)
    }

    pub(crate) fn apply_birth_date_change(&mut self, new_date: BirthDate) -> Result<bool> {
        if self.birth_date.as_ref() == Some(&new_date) {
            return Ok(false);
        }
        self.birth_date = Some(new_date);
        Ok(true)
    }

    pub(crate) fn apply_locale_change(&mut self, new_locale: Locale) -> Result<bool> {
        if self.locale == new_locale {
            return Ok(false);
        }
        self.locale = new_locale;
        Ok(true)
    }

    pub(crate) fn apply_registration(&mut self) -> Result<()> {
        self.state = AccountState::ACTIVE;
        self.last_active_at = Some(Utc::now());
        Ok(())
    }

    pub(crate) fn apply_active_state(&mut self) -> Result<bool> {
        if self.state == AccountState::ACTIVE {
            return Ok(false);
        }
        self.state = AccountState::ACTIVE;
        Ok(true)
    }

    pub(crate) fn apply_deactivation_state(&mut self) -> Result<bool> {
        if self.state == AccountState::DEACTIVATED {
            return Ok(false);
        }
        self.state = AccountState::DEACTIVATED;
        Ok(true)
    }

    pub(crate) fn apply_suspension_state(&mut self) -> Result<bool> {
        if self.state == AccountState::SUSPENDED {
            return Ok(false);
        }
        self.state = AccountState::SUSPENDED;
        Ok(true)
    }

    pub(crate) fn apply_unsuspend_state(&mut self) -> Result<bool> {
        if self.state != AccountState::SUSPENDED {
            return Ok(false);
        }
        self.state = AccountState::ACTIVE;
        Ok(true)
    }

    pub(crate) fn apply_ban_state(&mut self) -> Result<bool> {
        if self.state == AccountState::BANNED {
            return Ok(false);
        }
        self.state = AccountState::BANNED;
        Ok(true)
    }

    pub(crate) fn apply_unban_state(&mut self) -> Result<bool> {
        if self.state != AccountState::BANNED {
            return Ok(false);
        }
        self.state = AccountState::ACTIVE;
        Ok(true)
    }

    pub(crate) fn apply_activity_record(&mut self) -> Result<bool> {
        let now = Utc::now();
        if self
            .last_active_at
            .map_or(true, |l| now - l > Duration::minutes(5))
        {
            self.last_active_at = Some(now);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    // --- LOGIQUE DE LECTURE ---

    pub fn is_active(&self) -> bool {
        self.state == AccountState::ACTIVE
    }

    pub fn is_pending(&self) -> bool {
        self.state == AccountState::PENDING
    }

    pub fn is_deactivated(&self) -> bool {
        matches!(self.state, AccountState::DEACTIVATED)
    }

    pub fn is_banned(&self) -> bool {
        matches!(self.state, AccountState::BANNED)
    }

    pub fn is_suspended(&self) -> bool {
        matches!(self.state, AccountState::SUSPENDED)
    }

    pub fn is_blocked(&self) -> bool {
        matches!(
            self.state,
            AccountState::BANNED | AccountState::SUSPENDED | AccountState::DEACTIVATED
        )
    }
}

impl Entity for AccountIdentity {
    type Id = AccountId;

    fn id(&self) -> &Self::Id {
        &self.account_id
    }

    fn updated_at(&self) -> DateTime<Utc> {
        self.updated_at
    }

    fn entity_name() -> &'static str {
        "AccountIdentity"
    }

    fn map_constraint_to_field(constraint: &str) -> &'static str {
        match constraint {
            "account_identity_email_key" => "email",
            "account_identity_phone_number_key" => "phone_number",
            "account_identity_sub_id_key" => "sub_id",
            _ => "unique_constraint",
        }
    }
}
