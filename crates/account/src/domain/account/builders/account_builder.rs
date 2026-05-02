// crates/account/src/domain/account/builders/account_builder.rs

use crate::domain::value_objects::{
    AccountRole, AccountState, BirthDate, IpAddr,
};
use crate::domain::{
    account::{
        builders::{AccountGovernanceBuilder, AccountIdentityBuilder, AccountSettingsBuilder},
        entities::Account,
    },
    value_objects::{Locale, RegistrationIdentifier, TrustScore},
};
use shared_kernel::domain::value_objects::{Email, PhoneNumber, SubId, Timezone};
use shared_kernel::{
    domain::{
        events::AggregateMetadata,
        value_objects::{AccountId, RegionCode},
    },
    errors::Result,
};

pub struct AccountBuilder {
    identity: AccountIdentityBuilder,
    governance: AccountGovernanceBuilder,
    settings: AccountSettingsBuilder,
}

impl AccountBuilder {
    pub(crate) fn new(
        account_id: AccountId,
        region: RegionCode,
        identifier: RegistrationIdentifier,
    ) -> Self {
        let mut identity_builder = AccountIdentityBuilder::new(account_id.clone(), region);
        let governance_builder = AccountGovernanceBuilder::new(account_id.clone());
        let settings_builder = AccountSettingsBuilder::new(account_id);

        if let Some(email) = identifier.email() {
            identity_builder = identity_builder.with_email(email.clone());
        }

        if let Some(phone) = identifier.phone() {
            identity_builder = identity_builder.with_phone(phone.clone());
        }

        Self {
            identity: identity_builder,
            governance: governance_builder,
            settings: settings_builder,
        }
    }

    pub fn with_sub_id(mut self, sub_id: SubId) -> Self {
        self.identity = self.identity.with_sub_id(sub_id);
        self
    }

    pub fn with_locale(mut self, locale: Locale) -> Self {
        self.identity = self.identity.with_locale(locale);
        self
    }

    pub fn with_email(mut self, email: Email) -> Self {
        self.identity = self.identity.with_email(email);
        self
    }

    pub fn with_phone(mut self, phone: PhoneNumber) -> Self {
        self.identity = self.identity.with_phone(phone);
        self
    }

    pub fn with_birth_date(mut self, birth_date: BirthDate) -> Self {
        self.identity = self.identity.with_birth_date(birth_date);
        self
    }

    pub fn with_role(mut self, role: AccountRole) -> Self {
        self.governance = self.governance.with_role(role);
        self
    }

    pub fn with_ip_addr(mut self, ip: IpAddr) -> Self {
        self.governance = self.governance.with_ip_addr(ip);
        self
    }

    pub fn with_timezone(mut self, tz: Timezone) -> Self {
        self.settings = self.settings.with_timezone(tz);
        self
    }

    pub fn with_trust_score(mut self, score: TrustScore) -> Self {
        self.governance = self.governance.with_trust_score(score);
        self
    }

    pub fn with_state(mut self, state: AccountState) -> Self {
        self.identity = self.identity.with_state(state.clone());
        match state {
            AccountState::Banned => {
                self.governance = self
                    .governance
                    .with_trust_score(TrustScore::from_raw(TrustScore::MIN));
                self.governance = self.governance.with_shadowban(true);
            }
            AccountState::Suspended => {
                self.governance = self
                    .governance
                    .with_trust_score(TrustScore::from_raw(TrustScore::CRITICAL_THRESHOLD));
            }
            AccountState::Active | AccountState::Pending => {
                // On laisse le score par défaut (100) ou on ne touche à rien
            }
            AccountState::Deactivated => {
                // La désactivation n'impacte pas forcément le score
            }
        }
        self
    }

    pub fn identity<F>(mut self, f: F) -> Self
    where
        F: FnOnce(AccountIdentityBuilder) -> AccountIdentityBuilder,
    {
        self.identity = f(self.identity);
        self
    }

    pub fn governance<F>(mut self, f: F) -> Self
    where
        F: FnOnce(AccountGovernanceBuilder) -> AccountGovernanceBuilder,
    {
        self.governance = f(self.governance);
        self
    }

    pub fn settings<F>(mut self, f: F) -> Self
    where
        F: FnOnce(AccountSettingsBuilder) -> AccountSettingsBuilder,
    {
        self.settings = f(self.settings);
        self
    }

    pub fn build(self) -> Result<Account> {
        let metadata: AggregateMetadata = AggregateMetadata::default();
        Ok(Account::restore(
            self.identity.build()?,
            self.governance.build()?,
            self.settings.build()?,
            metadata,
        ))
    }
}
