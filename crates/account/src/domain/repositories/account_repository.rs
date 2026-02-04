// crates/account/src/domain/repositories/account_repository

use async_trait::async_trait;
use shared_kernel::domain::transaction::Transaction;
use shared_kernel::domain::value_objects::{AccountId, Username};
use shared_kernel::errors::Result;

use crate::domain::entities::Account;
use crate::domain::params::PatchUserParams;
use crate::domain::value_objects::{AccountState, Email, ExternalId, PhoneNumber};

#[async_trait]
pub trait AccountRepository: Send + Sync {
    // --- LECTURES OPTIMISÉES (PROJECTIONS) ---
    /// Récupère l'ID uniquement à partir d'un identifiant unique (Email/Username/Cognito).
    /// Très utile pour les jointures ou les redirections sans charger de données.
    async fn find_account_id_by_email(&self, email: &Email) -> Result<Option<AccountId>>;
    async fn find_account_id_by_username(&self, username: &Username) -> Result<Option<AccountId>>;
    async fn find_account_id_by_external_id(
        &self,
        external_id: &ExternalId,
        tx: Option<&mut dyn Transaction>,
    ) -> Result<Option<AccountId>>;

    // --- LECTURE COMPLÈTE ---

    /// Récupère l'entité User complète.
    async fn find_account_by_id(
        &self,
        id: &AccountId,
        tx: Option<&mut dyn Transaction>,
    ) -> Result<Option<Account>>;

    // --- VÉRIFICATIONS (OPTIMISATION DISQUE/RÉSEAU) ---

    async fn exists_account_by_email(&self, email: &Email) -> Result<bool>;
    async fn exists_account_by_username(&self, username: &Username) -> Result<bool>;
    async fn exists_account_by_phone_number(&self, phone_number: &PhoneNumber) -> Result<bool>;

    // --- ÉCRITURES TRANSACTIONNELLES (COMMANDES) ---

    /// Création initiale de l'utilisateur.
    /// Obligatoirement dans une transaction (car lié au Profile, Stats, etc.).
    async fn create_account(&self, account: &Account, tx: &mut dyn Transaction) -> Result<()>;

    /// Mise à jour partielle et dynamique (utilise le QueryBuilder).
    /// Permet de ne mettre à jour que ce qui a changé (ex: juste l'email).
    async fn patch_account_by_id(
        &self,
        account_id: &AccountId,
        params: PatchUserParams,
        tx: &mut dyn Transaction,
    ) -> Result<()>;
    async fn save(&self, user: &Account, tx: Option<&mut dyn Transaction>) -> Result<()>;

    /// Changement de statut atomique (Active -> Banned, etc.).
    async fn update_account_status_by_id(
        &self,
        account_id: &AccountId,
        account_state: AccountState,
        tx: &mut dyn Transaction,
    ) -> Result<()>;

    // --- OPÉRATIONS HAUTE FRÉQUENCE ---

    /// Mise à jour "Fire and Forget" de la dernière activité.
    /// Souvent appelé sans transaction pour maximiser le débit.
    async fn update_account_last_active(&self, account_id: &AccountId) -> Result<()>;

    /// Suppression définitive (RGPD).
    async fn delete(&self, id: &AccountId, tx: &mut dyn Transaction) -> Result<()>;
}
