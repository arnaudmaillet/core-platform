// crates/account/src/domain/repositories/global_identity_registry.rs (Suite)

use async_trait::async_trait;

use crate::types::AccountState;
use crate::types::RegistrationIdentifier;
use chrono::{DateTime, Utc};
use shared_kernel::{
    core::Result,
    types::{AccountId, Region, SubId},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlobalIdentityRegistration {
    pub account_id: AccountId,
    pub region: Region,
    pub sub_id: Option<SubId>,
    pub identifiers: RegistrationIdentifier,
    pub state: AccountState,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[async_trait]
pub trait GlobalIdentityRegistry: Send + Sync {
    /// 🚀 PHASE 1 : INSCRIPTION & VERROUILLAGE
    /// Tente d'insérer un nouvel enregistrement d'identité mondial.
    /// Doit retourner une `Error::ValidationError` ou `Error::ConcurrencyConflict`
    /// si l'un des index uniques mondiaux (`email_hash`, `phone_hash`, `sub_id`) est déjà pris.
    async fn reserve(&self, registration: &GlobalIdentityRegistration) -> Result<()>;

    /// 🔍 PHASE 2 : ROUTAGE & AUTHENTIFICATION (LOOKUP)
    /// Retrouve les informations de routage global complet d'un compte à partir de son ID.
    async fn find_by_account_id(
        &self,
        account_id: AccountId,
    ) -> Result<Option<GlobalIdentityRegistration>>;

    /// Retrouve la région et l'ID d'un compte à partir de son e-mail haché (SHA-256).
    /// Utile pour le flux de Login par Email ou pour la complétion des Read Models.
    async fn find_by_email_hash(
        &self,
        email_hash: &[u8],
    ) -> Result<Option<GlobalIdentityRegistration>>;

    /// Retrouve la région et l'ID d'un compte à partir de son numéro de téléphone haché (SHA-256).
    /// Utile pour le flux de Login par SMS / OTP.
    async fn find_by_phone_hash(
        &self,
        phone_hash: &[u8],
    ) -> Result<Option<GlobalIdentityRegistration>>;

    /// Retrouve la région et l'ID d'un compte à partir de son ID externe IdP (Keycloak `sub`).
    /// C'est le point d'entrée critique lors d'un Callback OAuth2/OIDC (Google, Apple, Keycloak).
    async fn find_by_sub_id(&self, sub_id: &str) -> Result<Option<GlobalIdentityRegistration>>;

    /// 🔄 PHASE 3 : MUTATION & CYCLE DE VIE
    /// Met à jour les identifiants mondiaux d'un compte (ex: l'utilisateur change d'e-mail ou ajoute un téléphone).
    /// Doit valider l'unicité des nouveaux hashs générés par le RegistrationIdentifier.
    async fn update_identifiers(
        &self,
        account_id: AccountId,
        new_identifiers: RegistrationIdentifier,
    ) -> Result<()>;

    /// Modifie l'état mondial du compte (ex: passe de UNVERIFIED à ACTIVE, ou SUSPENDED en cas de ban).
    /// Permet aux routeurs gRPC de rejeter un token avant même de charger le shard régional.
    async fn update_state(&self, account_id: AccountId, new_state: AccountState) -> Result<()>;

    /// 🗑️ PHASE 4 : NETTOYAGE & RGPD
    /// Supprime l'enregistrement d'identité mondial.
    /// Appelé uniquement à la fin du délai de carence légal d'effacement du compte.
    async fn delete(&self, account_id: AccountId) -> Result<()>;

    /// Purge les réservations mondiales restées en état 'PENDING' au-delà d'un certain délai.
    async fn purge_expired_reservations(
        &self,
        expired_before: chrono::DateTime<chrono::Utc>,
    ) -> Result<u64>;
}
