// crates/account/src/domain/builders/user_buidler.rs

use chrono::{DateTime, Utc};
use shared_kernel::domain::events::AggregateMetadata;
use shared_kernel::domain::value_objects::{RegionCode, AccountId, Username};
use crate::domain::entities::Account;
use crate::domain::value_objects::{ExternalId, Email, Locale, PhoneNumber, BirthDate, AccountState};

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
    /// Initialise les données obligatoires pour un nouveau compte.
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

    /// Chemin 2 : RESTAURATION (Via Repository / Infrastructure)
    /// Reconstruit l'objet complet sans aucune logique par défaut ni validation.
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
        Account {
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
            metadata: AggregateMetadata::restore(version)
        }
    }

    // --- SETTERS (Chemin Création) ---

    pub fn with_locale(mut self, locale: Locale) -> Self {
        self.locale = Some(locale);
        self
    }

    pub fn with_phone(mut self, phone: PhoneNumber) -> Self {
        self.phone = Some(phone);
        self
    }

    pub fn with_birth_date(mut self, birth_date: BirthDate) -> Self {
        self.birth_date = Some(birth_date);
        self
    }

    /// Finalise la création d'un NOUVEL utilisateur
    pub fn build(self) -> Account {
        let now = Utc::now();

        Account {
            id: self.id,
            region_code: self.region_code,
            external_id: self.external_id,
            username: self.username,
            email: self.email,
            email_verified: false, // Toujours false à la création
            phone_number: self.phone,
            phone_verified: false,
            account_state: AccountState::Pending, // État initial standard
            birth_date: self.birth_date,
            locale: self.locale.unwrap_or_default(),
            created_at: now,
            updated_at: now,
            last_active_at: Some(now), // L'utilisateur est actif à sa création
            metadata: AggregateMetadata::new(self.version),
        }
    }
}