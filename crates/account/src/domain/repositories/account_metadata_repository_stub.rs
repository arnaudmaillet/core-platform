use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use async_trait::async_trait;
use shared_kernel::domain::Identifier;
use shared_kernel::domain::transaction::Transaction;
use shared_kernel::domain::value_objects::AccountId;
use shared_kernel::errors::{DomainError, Result};

use crate::domain::entities::AccountMetadata;
use crate::domain::repositories::AccountMetadataRepository;

#[derive(Default)]
pub struct AccountMetadataRepositoryStub {
    /// Stockage en mémoire : AccountId -> AccountMetadata
    pub metadata_map: Arc<Mutex<HashMap<AccountId, AccountMetadata>>>,
    /// Simulation d'erreur forcée
    pub error_to_return: Arc<Mutex<Option<DomainError>>>,
}

impl AccountMetadataRepositoryStub {
    pub fn new() -> Self {
        Self::default()
    }

    /// Helper pour injecter des data avant un test
    pub fn add_metadata(&self, metadata: AccountMetadata) {
        self.metadata_map.lock().unwrap().insert(metadata.account_id().clone(), metadata);
    }

    fn check_error(&self) -> Result<()> {
        if let Some(err) = self.error_to_return.lock().unwrap().clone() {
            return Err(err);
        }
        Ok(())
    }

    fn not_found(&self, id: String) -> DomainError {
        DomainError::NotFound {
            entity: "AccountMetadata",
            id,
        }
    }
}

#[async_trait]
impl AccountMetadataRepository for AccountMetadataRepositoryStub {
    async fn find_by_account_id(&self, account_id: &AccountId) -> Result<Option<AccountMetadata>> {
        self.check_error()?;
        Ok(self.metadata_map.lock().unwrap().get(account_id).cloned())
    }

    async fn insert(&self, metadata: &AccountMetadata, _tx: &mut dyn Transaction) -> Result<()> {
        self.check_error()?;
        self.metadata_map.lock().unwrap().insert(metadata.account_id().clone(), metadata.clone());
        Ok(())
    }

    async fn save(
        &self,
        metadata: &AccountMetadata,
        _tx: Option<&mut dyn Transaction>,
    ) -> Result<()> {
        self.check_error()?;

        // Optionnel : simuler une erreur si on tente de save un truc qui n'existe pas
        // (comportement cohérent avec un UPDATE SQL qui ne toucherait aucune ligne)
        let mut map = self.metadata_map.lock().unwrap();
        if !map.contains_key(metadata.account_id()) {
            return Err(self.not_found(metadata.account_id().as_string()));
        }

        map.insert(metadata.account_id().clone(), metadata.clone());
        Ok(())
    }
}