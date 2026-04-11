// crates/account/src/domain/entities/account

use crate::domain::account::builders::AccountIdentityBuilder;
use crate::domain::events::AccountEvent;
use crate::domain::value_objects::{
    AccountState, BirthDate, Email, ExternalId, IpAddr, Locale, PhoneNumber,
};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use shared_kernel::domain::Identifier;
use shared_kernel::domain::entities::EntityMetadata;
use shared_kernel::domain::events::{AggregateMetadata, AggregateRoot};
use shared_kernel::domain::value_objects::{AccountId, RegionCode};
use shared_kernel::errors::{DomainError, Result};

/// Agrégat Racine User
///
/// Gère l'identité, la sécurité et le cycle de vie du compte.
/// Utilise AggregateMetadata pour l'Optimistic Concurrency Control et la capture d'événements.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountIdentity {
    account_id: AccountId,
    region_code: RegionCode,
    external_id: ExternalId,
    email: Email,
    email_verified: bool,
    phone_number: Option<PhoneNumber>,
    phone_verified: bool,
    state: AccountState,
    birth_date: Option<BirthDate>,
    locale: Locale,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    last_active_at: Option<DateTime<Utc>>,
    metadata: AggregateMetadata,
}

impl AccountIdentity {
    pub fn builder(
        account_id: AccountId,
        region_code: RegionCode,
        email: Email,
        external_id: ExternalId,
    ) -> AccountIdentityBuilder {
        AccountIdentityBuilder::new(account_id, region_code, email, external_id)
    }

    /// Utilisé par le Builder ou le Repository pour restaurer l'état
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn restore(
        account_id: AccountId,
        region_code: RegionCode,
        external_id: ExternalId,
        email: Email,
        email_verified: bool,
        phone_number: Option<PhoneNumber>,
        phone_verified: bool,
        state: AccountState,
        birth_date: Option<BirthDate>,
        locale: Locale,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
        last_active_at: Option<DateTime<Utc>>,
        metadata: AggregateMetadata,
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
            updated_at,
            last_active_at,
            metadata,
        }
    }

    // --- GETTERS PUBLICS ---

    pub fn account_id(&self) -> &AccountId {
        &self.account_id
    }
    pub fn region_code(&self) -> &RegionCode {
        &self.region_code
    }
    pub fn external_id(&self) -> &ExternalId {
        &self.external_id
    }
    pub fn email(&self) -> &Email {
        &self.email
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
    pub fn updated_at(&self) -> DateTime<Utc> {
        self.updated_at
    }
    pub fn last_active_at(&self) -> Option<DateTime<Utc>> {
        self.last_active_at
    }

    // ==========================================
    // GESTION DE L'IDENTITÉ (EMAIL & SÉCURITÉ)
    // ==========================================

    /// Lie une identité externe (ex: Cognito sub) au compte utilisateur.
    /// Cette opération est critique pour la sécurité.
    pub fn link_external_identity(&mut self, new_external_id: ExternalId) -> Result<bool> {
        if self.external_id == new_external_id {
            return Ok(false);
        }

        // Sécurité Hyperscale : Empêcher le double linkage si l'ID n'est pas vide
        // Note: Selon ton implémentation ExternalId, vérifie la méthode as_str() ou is_empty()
        if !self.external_id.as_str().is_empty() {
            return Err(DomainError::Forbidden {
                reason: "Account is already linked to an external provider".into(),
            });
        }
        let old_external_id = std::mem::replace(&mut self.external_id, new_external_id);
        self.apply_change();

        self.push_event(Box::new(AccountEvent::ExternalIdentityLinked {
            account_id: self.account_id.clone(),
            old_external_id,
            new_external_id: self.external_id.clone(),
            occurred_at: self.updated_at,
        }));

        Ok(true)
    }

    pub fn change_email(&mut self, new_email: Email) -> Result<bool> {
        if self.email == new_email {
            return Ok(false);
        }

        if self.is_blocked() {
            return Err(DomainError::Forbidden {
                reason: "Cannot change email of a restricted account".into(),
            });
        }

        let old_email = std::mem::replace(&mut self.email, new_email);
        self.email_verified = false;
        self.apply_change();

        self.push_event(Box::new(AccountEvent::EmailChanged {
            account_id: self.account_id.clone(),
            old_email: Some(old_email),
            new_email: self.email.clone(),
            occurred_at: self.updated_at,
        }));

        Ok(true)
    }

    pub fn verify_email(&mut self, token: &str) -> Result<bool> {
        if self.email_verified {
            return Ok(false);
        }

        // Note : Tu as probablement un champ 'verification_token' dans ton entité
        // ou une logique de signature/expiration.

        self.email_verified = true;
        self.apply_change();

        if self.state == AccountState::Pending {
            self.state = AccountState::Active;
        }

        self.push_event(Box::new(AccountEvent::EmailVerified {
            account_id: self.account_id.clone(),
            occurred_at: self.updated_at,
        }));

        Ok(true)
    }

    // ==========================================
    // GESTION DU TÉLÉPHONE (MFA / NOTIFICATIONS)
    // ==========================================

    pub fn change_phone_number(&mut self, new_phone: PhoneNumber) -> Result<bool> {
        if self.phone_number.as_ref() == Some(&new_phone) {
            return Ok(false);
        }

        let old_phone_number = std::mem::replace(&mut self.phone_number, Some(new_phone));
        self.phone_verified = false;
        self.apply_change();

        self.push_event(Box::new(AccountEvent::PhoneNumberChanged {
            account_id: self.account_id.clone(),
            old_phone_number,
            new_phone_number: self.phone_number.clone().unwrap(),
            occurred_at: self.updated_at,
        }));

        Ok(true)
    }

    pub fn verify_phone(&mut self, code: &str) -> Result<bool> {
        if self.phone_verified {
            return Ok(false);
        }

        // Note : Tu as probablement un champ 'verification_code' dans ton entité
        // ou une logique de signature/expiration.

        self.phone_verified = true;
        self.apply_change();

        self.push_event(Box::new(AccountEvent::PhoneVerified {
            account_id: self.account_id.clone(),
            occurred_at: self.updated_at,
        }));

        Ok(true)
    }

    // ==========================================
    // GESTION DU PROFIL & CONFORMITÉ
    // ==========================================

    pub fn change_birth_date(&mut self, new_date: BirthDate) -> Result<bool> {
        if self.birth_date.as_ref() == Some(&new_date) {
            return Ok(false);
        }

        if self.is_blocked() {
            return Err(DomainError::Forbidden {
                reason: "Cannot update restricted account profile".into(),
            });
        }

        self.birth_date = Some(new_date);
        self.apply_change();

        self.push_event(Box::new(AccountEvent::BirthDateChanged {
            account_id: self.account_id.clone(),
            occurred_at: self.updated_at,
        }));

        Ok(true)
    }

    pub fn update_locale(&mut self, new_locale: Locale) -> Result<bool> {
        if self.locale == new_locale {
            return Ok(false);
        }

        self.locale = new_locale;
        self.apply_change();

        self.push_event(Box::new(AccountEvent::LocaleUpdated {
            account_id: self.account_id.clone(),
            new_locale: self.locale.clone(),
            occurred_at: self.updated_at,
        }));

        Ok(true)
    }

    pub fn change_region(&mut self, new_region: RegionCode) -> Result<bool> {
        if self.region_code == new_region {
            return Ok(false);
        }

        let old_region = std::mem::replace(&mut self.region_code, new_region);
        self.apply_change();

        self.push_event(Box::new(AccountEvent::AccountRegionChanged {
            account_id: self.account_id.clone(),
            old_region,
            new_region: self.region_code.clone(),
            occurred_at: self.updated_at,
        }));

        Ok(true)
    }

    // ==========================================
    // CYCLE DE VIE & ÉTATS DE SÉCURITÉ
    // ==========================================

    pub fn register(&mut self, region: RegionCode, ip_addr: IpAddr) -> Result<bool> {
        self.state = AccountState::Active;
        self.last_active_at = Some(Utc::now());
        self.apply_change();

        self.push_event(Box::new(AccountEvent::AccountRegistered {
            account_id: self.account_id.clone(),
            email: self.email.clone(),
            external_id: self.external_id.clone(),
            locale: self.locale.clone(),
            region,
            ip_addr,
            occurred_at: self.updated_at,
        }));

        Ok(true)
    }

    pub fn deactivate(&mut self) -> Result<bool> {
        if self.state == AccountState::Deactivated {
            return Ok(false);
        }

        self.state = AccountState::Deactivated;
        self.apply_change();

        self.push_event(Box::new(AccountEvent::AccountDeactivated {
            account_id: self.account_id.clone(),
            occurred_at: self.updated_at,
        }));

        Ok(true)
    }

    pub fn activate(&mut self) -> Result<bool> {
        if self.is_active() {
            return Ok(false);
        }

        // Seul un compte désactivé par l'utilisateur peut être réactivé manuellement
        if self.state != AccountState::Deactivated {
            return Err(DomainError::Forbidden {
                reason: "Only deactivated accounts can be reactivated manually".into(),
            });
        }

        self.state = AccountState::Active;
        self.apply_change();

        self.push_event(Box::new(AccountEvent::AccountActivated {
            account_id: self.account_id.clone(),
            occurred_at: self.updated_at,
        }));

        Ok(true)
    }

    pub fn suspend(&mut self, reason: String) -> Result<bool> {
        if self.state == AccountState::Suspended {
            return Ok(false);
        }

        self.state = AccountState::Suspended;
        self.apply_change();

        self.push_event(Box::new(AccountEvent::AccountSuspended {
            account_id: self.account_id.clone(),
            reason,
            occurred_at: self.updated_at,
        }));

        Ok(true)
    }

    pub fn unsuspend(&mut self) -> Result<bool> {
        if self.state != AccountState::Suspended {
            return Ok(false);
        }

        self.state = AccountState::Active;
        self.apply_change();

        self.push_event(Box::new(AccountEvent::AccountUnsuspended {
            account_id: self.account_id.clone(),
            occurred_at: self.updated_at,
        }));

        Ok(true)
    }

    pub fn ban(&mut self, reason: &str) -> Result<bool> {
        if self.state == AccountState::Banned {
            return Ok(false);
        }

        self.state = AccountState::Banned;
        self.apply_change();
        self.push_event(Box::new(AccountEvent::AccountBanned {
            account_id: self.account_id.clone(),
            reason: reason.to_string(),
            occurred_at: self.updated_at,
        }));

        Ok(true)
    }

    pub fn unban(&mut self) -> Result<bool> {
        if self.state != AccountState::Banned {
            return Ok(false);
        }

        self.state = AccountState::Active;
        self.apply_change();

        self.push_event(Box::new(AccountEvent::AccountUnbanned {
            account_id: self.account_id.clone(),
            occurred_at: self.updated_at,
        }));

        Ok(true)
    }

    pub fn record_activity(&mut self) -> Result<bool> {
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

    // ==========================================
    // GETTERS DE LOGIQUE (READ-ONLY)
    // ==========================================

    pub fn is_blocked(&self) -> bool {
        matches!(
            self.state,
            AccountState::Banned | AccountState::Suspended | AccountState::Deactivated
        )
    }

    pub fn is_active(&self) -> bool {
        self.state == AccountState::Active
    }

    pub fn is_verified(&self) -> bool {
        self.email_verified || self.phone_verified
    }

    pub fn is_online(&self) -> bool {
        self.last_active_at
            .map(|last| Utc::now() - last < chrono::Duration::minutes(5))
            .unwrap_or(false)
    }

    pub fn can_login(&self) -> bool {
        self.state.can_authenticate()
    }

    // Helpers
    fn apply_change(&mut self) {
        self.increment_version();
        self.updated_at = Utc::now();
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

impl AggregateRoot for AccountIdentity {
    fn id(&self) -> String {
        self.account_id.as_string()
    }
    fn metadata(&self) -> &AggregateMetadata {
        &self.metadata
    }
    fn metadata_mut(&mut self) -> &mut AggregateMetadata {
        &mut self.metadata
    }
}
