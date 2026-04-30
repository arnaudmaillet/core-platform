// crates/account/src/domain/builders/_builder.rs

use crate::domain::account::entities::AccountIdentity;
use crate::domain::value_objects::{
    AccountState, BirthDate, Email, SubId, Locale, PhoneNumber,
};
use chrono::{DateTime, Utc};
use shared_kernel::domain::value_objects::{AccountId, RegionCode};
use shared_kernel::errors::{DomainError, Result};

pub struct AccountIdentityBuilder {
    account_id: AccountId,
    region_code: RegionCode,
    sub_id: Option<SubId>,
    email: Option<Email>,
    locale: Option<Locale>,
    phone: Option<PhoneNumber>,
    birth_date: Option<BirthDate>,
    state: AccountState,
    last_active_at: Option<DateTime<Utc>>,
}

impl AccountIdentityBuilder {
    pub(crate) fn new(
        account_id: AccountId,
        region_code: RegionCode,
    ) -> Self {
        Self {
            account_id,
            region_code,
            email: None,
            sub_id: None,
            locale: None,
            phone: None,
            birth_date: None,
            state: AccountState::Pending,
            last_active_at: None,
        }
    }

    // --- SETTERS ---

    pub fn with_locale(mut self, locale: Locale) -> Self {
        self.locale = Some(locale);
        self
    }

    pub fn with_optional_locale(mut self, locale: Option<Locale>) -> Self {
        self.locale = locale;
        self
    }

    pub fn with_email(mut self, email: Email) -> Self {
        self.email = Some(email);
        self
    }

    pub fn with_phone(mut self, phone: PhoneNumber) -> Self {
        self.phone = Some(phone);
        self
    }

    pub fn with_optional_phone(mut self, phone: Option<PhoneNumber>) -> Self {
        self.phone = phone;
        self
    }

    pub fn with_birth_date(mut self, birth_date: BirthDate) -> Self {
        self.birth_date = Some(birth_date);
        self
    }

    pub fn with_optional_birth_date(mut self, birth_date: Option<BirthDate>) -> Self {
        self.birth_date = birth_date;
        self
    }

    pub fn with_last_active_at(mut self, last_active: DateTime<Utc>) -> Self {
        self.last_active_at = Some(last_active);
        self
    }

    pub fn with_state(mut self, state: AccountState) -> Self {
        self.state = state;
        self
    }

    pub fn with_sub_id(mut self, sub_id: SubId) -> Self {
        self.sub_id = Some(sub_id);
        self
    }

    pub fn build(self) -> Result<AccountIdentity> {
        let now = Utc::now();

        if self.email.is_none() && self.phone.is_none() {
            return Err(DomainError::Validation {
                field: "identity",
                reason: "At least one contact method is required".into(),
            });
        }

        Ok(AccountIdentity::restore(
            self.account_id,
            self.region_code,
            self.sub_id,
            self.email,
            false,
            self.phone,
            false,
            self.state,
            self.birth_date,
            self.locale.unwrap_or_default(),
            now,
            now,
            now,
            self.last_active_at
        ))
    }
}
