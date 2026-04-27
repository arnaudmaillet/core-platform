// crates/account/src/domain/repositories/account_repository.rs

use async_trait::async_trait;
use shared_kernel::{
    domain::{transaction::Transaction, value_objects::AccountId},
    errors::Result,
};

use crate::domain::{
    account::entities::Account,
    value_objects::{Email, ExternalId, PhoneNumber},
};

#[async_trait]
pub trait AccountRepository: Send + Sync {
    // --- LECTURE ---

    /// Récupère l'agrégat complet à l'etat actuel du domain (incluant les changements non encore commit dans la transaction).
    async fn find_by_id(
        &self,
        id: &AccountId,
        tx: Option<&mut dyn Transaction>,
    ) -> Result<Option<Account>>;

    /// Récupère les valeurs à l'etat actuel du repository
    async fn find_id_by_email(&self, email: &Email) -> Result<Option<AccountId>>;
    async fn find_id_by_external_id(&self, ext_id: &ExternalId) -> Result<Option<AccountId>>;

    // --- VÉRIFICATIONS ---
    async fn exists_by_email(&self, email: &Email) -> Result<bool>;
    async fn exists_by_phone(&self, phone: &PhoneNumber) -> Result<bool>;
    async fn exists_by_external_id(&self, ext_id: &ExternalId) -> Result<bool>;

    // --- ÉCRITURE ---

    /// Sauvegarde de l'agrégat.
    async fn save(&self, account: &mut Account, tx: Option<&mut dyn Transaction>) -> Result<()>;

    /// Création initiale (pour le Register)
    async fn create(&self, account: &Account, tx: &mut dyn Transaction) -> Result<()>;

    async fn delete(&self, id: &AccountId, tx: &mut dyn Transaction) -> Result<()>;
}
