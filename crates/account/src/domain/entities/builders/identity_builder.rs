// crates/account/src/domain/builders/_builder.rs

use crate::entities::AccountIdentity;
use crate::types::{AccountState, BirthDate, Locale};
use chrono::{DateTime, Utc};
use shared_kernel::core::{Error, Result};
use shared_kernel::types::{AccountId, Email, Phone, SubId};

pub struct AccountIdentityBuilder {
    account_id: AccountId,
    sub_id: Option<SubId>,
    email: Option<Email>,
    locale: Option<Locale>,
    phone: Option<Phone>,
    birth_date: Option<BirthDate>,
    state: AccountState,
    last_active_at: Option<DateTime<Utc>>,
}

impl AccountIdentityBuilder {
    pub(crate) fn new(account_id: AccountId) -> Self {
        Self {
            account_id,
            email: None,
            sub_id: None,
            locale: None,
            phone: None,
            birth_date: None,
            state: AccountState::UNVERIFIED,
            last_active_at: None,
        }
    }

    // --- SETTERS ---

    pub fn with_account_id(mut self, account_id: AccountId) -> Self {
        self.account_id = account_id;
        self
    }

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

    pub fn with_phone(mut self, phone: Phone) -> Self {
        self.phone = Some(phone);
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
            return Err(Error::validation(
                "identity",
                "At least one contact method is required",
            ));
        }

        Ok(AccountIdentity::restore(
            self.account_id,
            self.sub_id,
            self.email,
            self.phone,
            self.state,
            self.birth_date,
            self.locale.unwrap_or_default(),
            now,
            now,
            now,
            self.last_active_at,
        ))
    }
}
