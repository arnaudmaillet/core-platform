// crates/account/src/domain/repositories/account_repository.rs

use async_trait::async_trait;
use shared_kernel::{
    core::{Result, Transaction},
    types::{AccountId, Email, PhoneNumber, Region, SubId},
};

use crate::domain::entities::Account;

#[async_trait]
pub trait AccountRepository: Send + Sync {
    async fn find_by_id(
        &self,
        region: Region,
        id: AccountId,
        tx: Option<&mut dyn Transaction>,
    ) -> Result<Option<Account>>;

    async fn find_by_sub_id(
        &self,
        region: Region,
        ext_id: &SubId,
        tx: Option<&mut dyn Transaction>,
    ) -> Result<Option<Account>>;

    async fn find_id_by_sub_id(
        &self,
        region: Region,
        ext_id: &SubId,
        tx: Option<&mut dyn Transaction>,
    ) -> Result<Option<AccountId>>;

    async fn find_id_by_email(
        &self,
        region: Region,
        email: &Email,
        tx: Option<&mut dyn Transaction>,
    ) -> Result<Option<AccountId>>;

    async fn exists_by_email(
        &self,
        region: Region,
        email: &Email,
        tx: Option<&mut dyn Transaction>,
    ) -> Result<bool>;

    async fn exists_by_phone(
        &self,
        region: Region,
        phone: &PhoneNumber,
        tx: Option<&mut dyn Transaction>,
    ) -> Result<bool>;

    async fn exists_by_sub_id(
        &self,
        region: Region,
        ext_id: &SubId,
        tx: Option<&mut dyn Transaction>,
    ) -> Result<bool>;

    // --- ÉCRITURE ---

    async fn save(
        &self,
        region: Region,
        account: &mut Account,
        tx: Option<&mut dyn Transaction>,
    ) -> Result<()>;

    async fn create(
        &self,
        region: Region,
        account: &Account,
        tx: &mut dyn Transaction,
    ) -> Result<()>;

    async fn delete(&self, region: Region, id: AccountId, tx: &mut dyn Transaction) -> Result<()>;
}
