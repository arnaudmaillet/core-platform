// crates/account/src/domain/repositories/account_repository.rs

use async_trait::async_trait;
use shared_kernel::domain::transaction::Transaction;
use shared_kernel::domain::value_objects::{AccountId, Username};
use shared_kernel::errors::Result;

use crate::domain::account::entities::AccountIdentity;
use crate::domain::value_objects::{AccountState, Email, ExternalId, PhoneNumber};

#[async_trait]
pub trait AccountIdentityRepository: Send + Sync {
    async fn fetch_by_id(&self, id: &AccountId, tx: Option<&mut dyn Transaction>) -> Result<Option<AccountIdentity>>;
    async fn resolve_id_from_external_id(&self, ext_id: &ExternalId) -> Result<Option<AccountId>>;
    async fn resolve_id_from_email(&self, email: &Email) -> Result<Option<AccountId>>;
    async fn exists_by_external_id(&self, ext_id: &ExternalId) -> Result<bool>;
    async fn exists_by_email(&self, email: &Email) -> Result<bool>;
    async fn exists_by_phone(&self, phone: &PhoneNumber) -> Result<bool>;
    async fn save(&self, account: &AccountIdentity, original: Option<&AccountIdentity>, tx: Option<&mut dyn Transaction>) -> Result<()>;
    async fn transit_to_state(&self, id: &AccountId, state: AccountState, tx: &mut dyn Transaction) -> Result<()>;
    async fn record_activity(&self, id: &AccountId) -> Result<()>;
    async fn hard_delete(&self, id: &AccountId, tx: &mut dyn Transaction) -> Result<()>;
}