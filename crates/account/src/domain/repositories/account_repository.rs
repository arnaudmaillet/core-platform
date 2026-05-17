// crates/account/src/domain/repositories/account_repository.rs

use async_trait::async_trait;
use shared_kernel::{
    core::{Result, Transaction},
    types::{AccountId, Email, PhoneNumber, SubId},
};

use crate::domain::entities::Account;

#[async_trait]
pub trait AccountRepository: Send + Sync {
    // --- LECTURE ---

    async fn find_by_id(
        &self,
        id: AccountId,
        tx: Option<&mut dyn Transaction>,
    ) -> Result<Option<Account>>;

    async fn find_by_sub_id(
        &self,
        ext_id: &SubId,
        tx: Option<&mut dyn Transaction>,
    ) -> Result<Option<Account>>;

    async fn find_id_by_sub_id(
        &self,
        ext_id: &SubId,
        tx: Option<&mut dyn Transaction>,
    ) -> Result<Option<AccountId>>;

    // AJOUTER tx ICI
    async fn find_id_by_email(
        &self,
        email: &Email,
        tx: Option<&mut dyn Transaction>,
    ) -> Result<Option<AccountId>>;

    // --- VÉRIFICATIONS (AJOUTER tx PARTOUT) ---
    async fn exists_by_email(
        &self,
        email: &Email,
        tx: Option<&mut dyn Transaction>,
    ) -> Result<bool>;

    async fn exists_by_phone(
        &self,
        phone: &PhoneNumber,
        tx: Option<&mut dyn Transaction>,
    ) -> Result<bool>;

    async fn exists_by_sub_id(
        &self,
        ext_id: &SubId,
        tx: Option<&mut dyn Transaction>,
    ) -> Result<bool>;

    // --- ÉCRITURE ---
    async fn save(&self, account: &mut Account, tx: Option<&mut dyn Transaction>) -> Result<()>;
    async fn create(&self, account: &Account, tx: &mut dyn Transaction) -> Result<()>;
    async fn delete(&self, id: AccountId, tx: &mut dyn Transaction) -> Result<()>;
}
