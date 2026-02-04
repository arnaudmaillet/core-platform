// crates/account/src/domain/entities/account

use crate::domain::builders::AccountBuilder;
use crate::domain::events::AccountEvent;
use crate::domain::value_objects::{
    AccountState, BirthDate, Email, ExternalId, Locale, PhoneNumber,
};
use chrono::{DateTime, Utc};
use shared_kernel::domain::Identifier;
use shared_kernel::domain::entities::EntityMetadata;
use shared_kernel::domain::events::{AggregateMetadata, AggregateRoot};
use shared_kernel::domain::value_objects::{AccountId, RegionCode, Username};
use shared_kernel::errors::{DomainError, Result};

/// Agrégat Racine User
///
/// Gère l'identité, la sécurité et le cycle de vie du compte.
/// Utilise AggregateMetadata pour l'Optimistic Concurrency Control et la capture d'événements.
#[derive(Debug, Clone)]
pub struct Account {
    pub id: AccountId,
    pub region_code: RegionCode,
    pub external_id: ExternalId,
    pub username: Username,
    pub email: Email,
    pub email_verified: bool,
    pub phone_number: Option<PhoneNumber>,
    pub phone_verified: bool,
    pub account_state: AccountState,
    pub birth_date: Option<BirthDate>,
    pub locale: Locale,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_active_at: Option<DateTime<Utc>>,
    pub metadata: AggregateMetadata,
}

impl Account {
    pub fn builder(
        id: AccountId,
        region_code: RegionCode,
        username: Username,
        email: Email,
        external_id: ExternalId,
    ) -> AccountBuilder {
        AccountBuilder::new(id, region_code, username, email, external_id)
    }

    // ==========================================
    // GESTION DE L'IDENTITÉ (EMAIL & SÉCURITÉ)
    // ==========================================

    /// Lie une identité externe (ex: Cognito sub) au compte utilisateur.
    /// Cette opération est critique pour la sécurité.
    pub fn link_external_identity(&mut self, external_id: ExternalId) -> Result<()> {
        if self.external_id == external_id {
            return Ok(());
        }

        // Sécurité Hyperscale : Empêcher le double linkage si l'ID n'est pas vide
        // Note: Selon ton implémentation ExternalId, vérifie la méthode as_str() ou is_empty()
        if !self.external_id.as_str().is_empty() {
            return Err(DomainError::Forbidden {
                reason: "Account is already linked to an external provider".into(),
            });
        }

        self.external_id = external_id.clone();
        self.apply_change();

        self.add_event(Box::new(AccountEvent::ExternalIdentityLinked {
            account_id: self.id.clone(),
            region: self.region_code.clone(),
            external_id,
            occurred_at: self.updated_at,
        }));

        Ok(())
    }

    pub fn change_email(&mut self, new_email: Email) -> Result<()> {
        if self.email == new_email {
            return Ok(());
        }

        if self.is_blocked() {
            return Err(DomainError::Forbidden {
                reason: "Cannot change email of a restricted account".into(),
            });
        }

        let old_email = Some(self.email.clone());
        self.email = new_email.clone();
        self.email_verified = false;
        self.apply_change();

        self.add_event(Box::new(AccountEvent::EmailChanged {
            account_id: self.id.clone(),
            region: self.region_code.clone(),
            old_email,
            new_email,
            occurred_at: self.updated_at,
        }));

        Ok(())
    }

    pub fn verify_email(&mut self) -> Result<()> {
        if self.email_verified {
            return Ok(());
        }

        self.email_verified = true;
        self.apply_change();

        if self.account_state == AccountState::Pending {
            self.account_state = AccountState::Active;
        }

        self.add_event(Box::new(AccountEvent::EmailVerified {
            account_id: self.id.clone(),
            region: self.region_code.clone(),
            occurred_at: self.updated_at,
        }));

        Ok(())
    }

    // ==========================================
    // GESTION DU TÉLÉPHONE (MFA / NOTIFICATIONS)
    // ==========================================

    pub fn change_phone_number(&mut self, new_phone: PhoneNumber) -> Result<()> {
        if self.phone_number.as_ref() == Some(&new_phone) {
            return Ok(());
        }

        let old_phone_number = self.phone_number.clone();
        self.phone_number = Some(new_phone.clone());
        self.phone_verified = false;
        self.apply_change();

        self.add_event(Box::new(AccountEvent::PhoneNumberChanged {
            account_id: self.id.clone(),
            region: self.region_code.clone(),
            old_phone_number,
            new_phone_number: new_phone,
            occurred_at: self.updated_at,
        }));

        Ok(())
    }

    pub fn verify_phone(&mut self) -> Result<()> {
        if self.phone_verified {
            return Ok(());
        }

        self.phone_verified = true;
        self.apply_change();

        self.add_event(Box::new(AccountEvent::PhoneVerified {
            account_id: self.id.clone(),
            region: self.region_code.clone(),
            occurred_at: self.updated_at,
        }));

        Ok(())
    }

    // ==========================================
    // GESTION DU PROFIL & CONFORMITÉ
    // ==========================================

    pub fn change_username(&mut self, new_username: Username) -> Result<()> {
        if self.username == new_username {
            return Ok(());
        }

        if self.is_blocked() {
            return Err(DomainError::Forbidden {
                reason: "Cannot change username of a restricted account".into(),
            });
        }

        let old_username = self.username.clone();
        self.username = new_username.clone();
        self.apply_change();

        self.add_event(Box::new(AccountEvent::UsernameChanged {
            account_id: self.id.clone(),
            region: self.region_code.clone(),
            old_username,
            new_username,
            occurred_at: self.updated_at,
        }));

        Ok(())
    }

    pub fn change_birth_date(&mut self, new_date: BirthDate) -> Result<()> {
        if self.birth_date.as_ref() == Some(&new_date) {
            return Ok(());
        }

        if self.is_blocked() {
            return Err(DomainError::Forbidden {
                reason: "Cannot update restricted account profile".into(),
            });
        }

        self.birth_date = Some(new_date);
        self.apply_change();

        self.add_event(Box::new(AccountEvent::BirthDateChanged {
            account_id: self.id.clone(),
            region: self.region_code.clone(),
            occurred_at: self.updated_at,
        }));

        Ok(())
    }

    pub fn update_locale(&mut self, new_locale: Locale) -> Result<()> {
        if self.locale == new_locale {
            return Ok(());
        }

        self.locale = new_locale;
        self.apply_change();

        self.add_event(Box::new(AccountEvent::LocaleChanged {
            account_id: self.id.clone(),
            region: self.region_code.clone(),
            new_locale: self.locale.clone(),
            occurred_at: self.updated_at,
        }));

        Ok(())
    }

    pub fn change_region(&mut self, new_region: RegionCode) -> Result<()> {
        if self.region_code == new_region {
            return Ok(());
        }

        let old_region = self.region_code.clone();
        self.region_code = new_region.clone();
        self.apply_change();

        self.add_event(Box::new(AccountEvent::AccountRegionChanged {
            account_id: self.id.clone(),
            old_region,
            new_region,
            occurred_at: self.updated_at,
        }));

        Ok(())
    }

    // ==========================================
    // CYCLE DE VIE & ÉTATS DE SÉCURITÉ
    // ==========================================

    pub fn deactivate(&mut self) -> Result<()> {
        if self.account_state == AccountState::Deactivated {
            return Ok(());
        }

        self.account_state = AccountState::Deactivated;
        self.apply_change();

        self.add_event(Box::new(AccountEvent::AccountDeactivated {
            account_id: self.id.clone(),
            region: self.region_code.clone(),
            occurred_at: self.updated_at,
        }));

        Ok(())
    }

    pub fn reactivate(&mut self) -> Result<()> {
        if self.is_active() {
            return Ok(());
        }

        // Seul un compte désactivé par l'utilisateur peut être réactivé manuellement
        if self.account_state != AccountState::Deactivated {
            return Err(DomainError::Forbidden {
                reason: "Only deactivated accounts can be reactivated manually".into(),
            });
        }

        self.account_state = AccountState::Active;
        self.apply_change();

        self.add_event(Box::new(AccountEvent::AccountReactivated {
            account_id: self.id.clone(),
            region: self.region_code.clone(),
            occurred_at: self.updated_at,
        }));

        Ok(())
    }

    pub fn suspend(&mut self, reason: String) -> Result<()> {
        if self.account_state == AccountState::Suspended {
            return Ok(());
        }

        self.account_state = AccountState::Suspended;
        self.apply_change();

        self.add_event(Box::new(AccountEvent::AccountSuspended {
            account_id: self.id.clone(),
            region: self.region_code.clone(),
            reason,
            occurred_at: self.updated_at,
        }));

        Ok(())
    }

    pub fn unsuspend(&mut self) -> Result<()> {
        if self.account_state != AccountState::Suspended {
            return Ok(());
        }

        self.account_state = AccountState::Active;
        self.apply_change();

        self.add_event(Box::new(AccountEvent::AccountUnsuspended {
            account_id: self.id.clone(),
            region: self.region_code.clone(),
            occurred_at: self.updated_at,
        }));

        Ok(())
    }

    pub fn ban(&mut self, reason: String) -> Result<()> {
        if self.account_state == AccountState::Banned {
            return Ok(());
        }

        self.account_state = AccountState::Banned;
        self.apply_change();
        self.add_event(Box::new(AccountEvent::AccountBanned {
            account_id: self.id.clone(),
            region: self.region_code.clone(),
            reason,
            occurred_at: self.updated_at,
        }));

        Ok(())
    }

    pub fn unban(&mut self) -> Result<()> {
        if self.account_state != AccountState::Banned {
            return Ok(());
        }

        self.account_state = AccountState::Active;
        self.apply_change();

        self.add_event(Box::new(AccountEvent::AccountUnbanned {
            account_id: self.id.clone(),
            region: self.region_code.clone(),
            occurred_at: self.updated_at,
        }));

        Ok(())
    }

    pub fn record_activity(&mut self) {
        let now = Utc::now();
        //  On ne met à jour en DB que toutes les 5 minutes
        if self
            .last_active_at
            .map_or(true, |last| now - last > chrono::Duration::minutes(5))
        {
            self.last_active_at = Some(now);
        }
    }

    // ==========================================
    // GETTERS DE LOGIQUE (READ-ONLY)
    // ==========================================

    pub fn is_blocked(&self) -> bool {
        matches!(
            self.account_state,
            AccountState::Banned | AccountState::Suspended | AccountState::Deactivated
        )
    }

    pub fn is_active(&self) -> bool {
        self.account_state == AccountState::Active
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
        self.account_state.can_authenticate()
    }

    // Helpers
    fn apply_change(&mut self) {
        self.increment_version();
        self.updated_at = Utc::now();
    }
}

impl EntityMetadata for Account {
    fn entity_name() -> &'static str {
        "User"
    }

    fn map_constraint_to_field(constraint: &str) -> &'static str {
        match constraint {
            "users_email_key" => "email",
            "users_username_key" => "username",
            "users_phone_number_key" => "phone_number",
            "users_external_id_key" => "external_id",
            _ => "unique_constraint",
        }
    }
}

impl AggregateRoot for Account {
    fn id(&self) -> String {
        self.id.as_string()
    }
    fn metadata(&self) -> &AggregateMetadata {
        &self.metadata
    }
    fn metadata_mut(&mut self) -> &mut AggregateMetadata {
        &mut self.metadata
    }
}
