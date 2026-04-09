// crates/account/src/domain/repositories/account_metadata_repository.rs

use async_trait::async_trait;
use shared_kernel::domain::transaction::Transaction;
use shared_kernel::domain::value_objects::AccountId;
use shared_kernel::errors::Result;

use crate::domain::account::entities::AccountMetadata;

#[async_trait]
pub trait AccountMetadataRepository: Send + Sync {
    async fn fetch_by_account_id(
        &self, 
        account_id: &AccountId,
        mut tx: Option<&mut dyn Transaction>
    ) -> Result<Option<AccountMetadata>>;
    async fn save(
        &self,
        metadata: &AccountMetadata,
        original: Option<&AccountMetadata>,
        mut tx: Option<&mut dyn Transaction>,
    ) -> Result<()>;
}
