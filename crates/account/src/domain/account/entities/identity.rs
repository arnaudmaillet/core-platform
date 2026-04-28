// crates/account/src/domain/entities/identity.rs

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use shared_kernel::{
    domain::{
        entities::EntityMetadata,
        value_objects::{AccountId, RegionCode},
    },
    errors::Result,
};

use crate::domain::{
    account::builders::AccountIdentityBuilder,
    value_objects::{
        AccountState, BirthDate, Email, ExternalId, Locale, PhoneNumber, VerificationCode,
        VerificationToken,
    },
};

/// Entité Identity (Interne à l'Agrégat Account)
///
/// Gère les données brutes d'identification et les transitions d'état local.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountIdentity {
    account_id: AccountId,
    region_code: RegionCode,
    external_id: Option<ExternalId>,
    email: Option<Email>,
    email_verified: bool,
    phone_number: Option<PhoneNumber>,
    phone_verified: bool,
    state: AccountState,
    birth_date: Option<BirthDate>,
    locale: Locale,
    created_at: DateTime<Utc>,
    last_active_at: Option<DateTime<Utc>>,
}

impl AccountIdentity {
    pub fn builder(
        account_id: AccountId,
        region_code: RegionCode,
    ) -> AccountIdentityBuilder {
        AccountIdentityBuilder::new(account_id, region_code)
    }

    /// Utilisé par le Builder ou le Repository pour restaurer l'état.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn restore(
        account_id: AccountId,
        region_code: RegionCode,
        external_id: Option<ExternalId>,
        email: Option<Email>,
        email_verified: bool,
        phone_number: Option<PhoneNumber>,
        phone_verified: bool,
        state: AccountState,
        birth_date: Option<BirthDate>,
        locale: Locale,
        created_at: DateTime<Utc>,
        last_active_at: Option<DateTime<Utc>>,
    ) -> Self {
        Self {
            account_id,
            region_code,
            external_id,
            email,
            email_verified,
            phone_number,
            phone_verified,
            state,
            birth_date,
            locale,
            created_at,
            last_active_at,
        }
    }

    // --- GETTERS ---

    pub fn account_id(&self) -> &AccountId {
        &self.account_id
    }
    pub fn region_code(&self) -> &RegionCode {
        &self.region_code
    }
    pub fn external_id(&self) -> Option<&ExternalId> {
        self.external_id.as_ref()
    }
    pub fn email(&self) -> Option<&Email> {
        self.email.as_ref()
    }
    pub fn is_email_verified(&self) -> bool {
        self.email_verified
    }
    pub fn phone_number(&self) -> Option<&PhoneNumber> {
        self.phone_number.as_ref()
    }
    pub fn is_phone_verified(&self) -> bool {
        self.phone_verified
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
    pub fn last_active_at(&self) -> Option<DateTime<Utc>> {
        self.last_active_at
    }

    // ==========================================
    // MUTATIONS INTERNES (pub(crate))
    // ==========================================

    pub(crate) fn apply_external_id_change(&mut self, new_external_id: ExternalId) -> Result<bool> {
        if self.external_id.as_ref() == Some(&new_external_id) {
            return Ok(false);
        }
        self.external_id = Some(new_external_id);
        Ok(true)
    }

    pub(crate) fn apply_email_change(&mut self, new_email: Email) -> Result<bool> {
        if self.email.as_ref() == Some(&new_email) {
            return Ok(false);
        }
        self.email = Some(new_email);
        self.email_verified = false;
        Ok(true)
    }

    pub(crate) fn apply_email_verification(&mut self) -> Result<bool> {
        if self.email_verified {
            return Ok(false);
        }
        self.email_verified = true;
        if self.state == AccountState::Pending {
            self.state = AccountState::Active;
        }
        Ok(true)
    }

    pub(crate) fn apply_phone_change(&mut self, new_phone: PhoneNumber) -> Result<bool> {
        if self.phone_number.as_ref() == Some(&new_phone) {
            return Ok(false);
        }
        self.phone_number = Some(new_phone);
        self.phone_verified = false;
        Ok(true)
    }

    pub(crate) fn apply_phone_verification(&mut self) -> Result<bool> {
        if self.phone_verified {
            return Ok(false);
        }
        self.phone_verified = true;
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
        self.state = AccountState::Active;
        self.last_active_at = Some(Utc::now());
        Ok(())
    }

    pub(crate) fn apply_active_state(&mut self) -> Result<bool> {
        if self.state == AccountState::Active {
            return Ok(false);
        }
        self.state = AccountState::Active;
        Ok(true)
    }

    pub(crate) fn apply_deactivation_state(&mut self) -> Result<bool> {
        if self.state == AccountState::Deactivated {
            return Ok(false);
        }
        self.state = AccountState::Deactivated;
        Ok(true)
    }

    pub(crate) fn apply_suspension_state(&mut self) -> Result<bool> {
        if self.state == AccountState::Suspended {
            return Ok(false);
        }
        self.state = AccountState::Suspended;
        Ok(true)
    }

    pub(crate) fn apply_unsuspend_state(&mut self) -> Result<bool> {
        if self.state != AccountState::Suspended {
            return Ok(false);
        }
        self.state = AccountState::Active;
        Ok(true)
    }

    pub(crate) fn apply_ban_state(&mut self) -> Result<bool> {
        if self.state == AccountState::Banned {
            return Ok(false);
        }
        self.state = AccountState::Banned;
        Ok(true)
    }

    pub(crate) fn apply_unban_state(&mut self) -> Result<bool> {
        if self.state != AccountState::Banned {
            return Ok(false);
        }
        self.state = AccountState::Active;
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
        self.state == AccountState::Active
    }

    pub fn is_pending(&self) -> bool {
        self.state == AccountState::Pending
    }

    pub fn is_deactivated(&self) -> bool {
        matches!(self.state, AccountState::Deactivated)
    }

    pub fn is_banned(&self) -> bool {
        matches!(self.state, AccountState::Banned)
    }

    pub fn is_suspended(&self) -> bool {
        matches!(self.state, AccountState::Suspended)
    }

    pub fn is_blocked(&self) -> bool {
        matches!(
            self.state,
            AccountState::Banned | AccountState::Suspended | AccountState::Deactivated
        )
    }
}

impl EntityMetadata for AccountIdentity {
    fn entity_name() -> &'static str {
        "AccountIdentity"
    }

    fn map_constraint_to_field(constraint: &str) -> &'static str {
        match constraint {
            "account_identity_email_key" => "email",
            "account_identity_phone_number_key" => "phone_number",
            "account_identity_external_id_key" => "external_id",
            _ => "unique_constraint",
        }
    }
}
