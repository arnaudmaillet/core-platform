// crates/account/src/domain/repositories/account_metadata_repository.rs

use async_trait::async_trait;
use shared_kernel::domain::transaction::Transaction;
use shared_kernel::domain::value_objects::AccountId;
use shared_kernel::errors::Result;

use crate::domain::account::entities::AccountMetadata;

#[async_trait]
pub trait AccountMetadataRepository: Send + Sync {
    async fn fetch_by_account_id(&self, id: &AccountId) -> Result<Option<AccountMetadata>>;
    async fn save(
        &self,
        metadata: &AccountMetadata,
        original: Option<&AccountMetadata>,
        tx: Option<&mut dyn Transaction>,
    ) -> Result<()>;
}
