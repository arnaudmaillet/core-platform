// crates/account/src/domain/repositories/account_metadata_repository.rs

use async_trait::async_trait;
use shared_kernel::domain::transaction::Transaction;
use shared_kernel::domain::value_objects::AccountId;
use shared_kernel::errors::Result;

use crate::domain::entities::AccountMetadata;

#[async_trait]
pub trait AccountMetadataRepository: Send + Sync {
    /// Récupère les métadonnées système d'un compte.
    async fn find_by_account_id(&self, account_id: &AccountId) -> Result<Option<AccountMetadata>>;

    /// Insère les métadonnées initiales (utilisé lors de l'inscription).
    async fn insert(&self, metadata: &AccountMetadata, tx: &mut dyn Transaction) -> Result<()>;

    /// Met à jour l'intégralité des métadonnées.
    async fn save(
        &self,
        metadata: &AccountMetadata,
        tx: Option<&mut dyn Transaction>,
    ) -> Result<()>;
}
