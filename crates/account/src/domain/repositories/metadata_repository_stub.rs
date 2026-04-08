// crates/account/src/domain/repositories/stubs/account_metadata_repository_stub.rs

use async_trait::async_trait;
use shared_kernel::domain::Identifier;
use shared_kernel::domain::events::AggregateRoot;
use shared_kernel::domain::transaction::Transaction;
use shared_kernel::domain::value_objects::AccountId;
use shared_kernel::errors::{DomainError, Result};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::domain::account::entities::AccountMetadata;
use crate::domain::repositories::AccountMetadataRepository;

#[derive(Default)]
pub struct AccountMetadataRepositoryStub {
    pub metadata_map: Arc<Mutex<HashMap<AccountId, AccountMetadata>>>,
    pub error_to_return: Arc<Mutex<Option<DomainError>>>,
}

impl AccountMetadataRepositoryStub {
    pub fn new() -> Self {
        Self::default()
    }

    /// Helper pour injecter des données initiales dans les tests
    pub fn add_metadata(&self, metadata: AccountMetadata) {
        self.metadata_map
            .lock()
            .unwrap()
            .insert(metadata.account_id().clone(), metadata);
    }

    fn check_error(&self) -> Result<()> {
        if let Some(err) = self.error_to_return.lock().unwrap().clone() {
            return Err(err);
        }
        Ok(())
    }
}

#[async_trait]
impl AccountMetadataRepository for AccountMetadataRepositoryStub {
    async fn fetch_by_account_id(&self, id: &AccountId) -> Result<Option<AccountMetadata>> {
        self.check_error()?;
        Ok(self.metadata_map.lock().unwrap().get(id).cloned())
    }

    async fn save(
        &self,
        metadata: &AccountMetadata,
        original: Option<&AccountMetadata>,
        _tx: Option<&mut dyn Transaction>,
    ) -> Result<()> {
        self.check_error()?;

        let mut map = self.metadata_map.lock().unwrap();
        let account_id = metadata.account_id();

        match original {
            Some(orig) => {
                let current_in_map = map.get(account_id).ok_or_else(|| DomainError::NotFound {
                    entity: "AccountMetadata",
                    id: account_id.as_string(),
                })?;

                // Vérification stricte de la version
                if current_in_map.version() != orig.version() {
                    return Err(DomainError::ConcurrencyConflict {
                        reason: format!(
                            "AccountMetadata OCC Conflict: Stub has v{}, but you tried to update v{}",
                            current_in_map.version(),
                            orig.version()
                        ),
                    });
                }
            }
            None => {
                if map.contains_key(account_id) {
                    return Err(DomainError::AlreadyExists {
                        entity: "AccountMetadata",
                        field: "account_id",
                        value: account_id.as_string(),
                    });
                }
            }
        }

        map.insert(account_id.clone(), metadata.clone());
        Ok(())
    }
}
