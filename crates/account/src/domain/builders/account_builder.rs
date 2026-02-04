// crates/account/src/domain/builders/account_builder.rs

use crate::domain::entities::Account;
use crate::domain::value_objects::{
    AccountState, BirthDate, Email, ExternalId, Locale, PhoneNumber,
};
use chrono::{DateTime, Utc};
use shared_kernel::domain::events::AggregateMetadata;
use shared_kernel::domain::value_objects::{AccountId, RegionCode, Username};

pub struct AccountBuilder {
    id: AccountId,
    region_code: RegionCode,
    external_id: ExternalId,
    username: Username,
    email: Email,
    locale: Option<Locale>,
    phone: Option<PhoneNumber>,
    birth_date: Option<BirthDate>,
    version: i32,
}

impl AccountBuilder {
    /// Chemin 1 : CRÉATION (Via Use Case d'inscription)
    pub fn new(
        id: AccountId,
        region_code: RegionCode,
        username: Username,
        email: Email,
        external_id: ExternalId,
    ) -> Self {
        Self {
            id,
            region_code,
            username,
            email,
            external_id,
            locale: None,
            phone: None,
            birth_date: None,
            version: 1,
        }
    }

    /// Chemin 2 : RESTAURATION (Via Repository)
    /// Utilise la méthode statique de Account pour reconstruire l'agrégat.
    #[allow(clippy::too_many_arguments)]
    pub fn restore(
        id: AccountId,
        region_code: RegionCode,
        external_id: ExternalId,
        username: Username,
        email: Email,
        email_verified: bool,
        phone_number: Option<PhoneNumber>,
        phone_verified: bool,
        account_state: AccountState,
        birth_date: Option<BirthDate>,
        locale: Locale,
        version: i32,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
        last_active_at: Option<DateTime<Utc>>,
    ) -> Account {
        // On appelle la méthode restore de l'entité Account
        Account::restore(
            id,
            region_code,
            external_id,
            username,
            email,
            email_verified,
            phone_number,
            phone_verified,
            account_state,
            birth_date,
            locale,
            created_at,
            updated_at,
            last_active_at,
            AggregateMetadata::restore(version),
        )
    }

    // --- SETTERS FLUIDES ---

    pub fn with_locale(mut self, locale: Locale) -> Self {
        self.locale = Some(locale);
        self
    }

    pub fn with_optional_locale(mut self, locale: Option<Locale>) -> Self {
        self.locale = locale;
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

    /// Finalise la création d'un NOUVEL utilisateur
    pub fn build(self) -> Account {
        let now = Utc::now();

        // On utilise la même méthode restore en interne pour garantir
        // que l'instanciation de l'agrégat est centralisée.
        Account::restore(
            self.id,
            self.region_code,
            self.external_id,
            self.username,
            self.email,
            false,
            self.phone,
            false,
            AccountState::Pending,
            self.birth_date,
            self.locale.unwrap_or_default(),
            now,
            now,
            Some(now),
            AggregateMetadata::new(self.version),
        )
    }
}