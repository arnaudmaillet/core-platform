// crates/account/src/domain/repositories/stubs/account_metadata_repository_stub.rs

use async_trait::async_trait;
use shared_kernel::domain::events::AggregateRoot;
use shared_kernel::domain::transaction::Transaction;
use shared_kernel::domain::value_objects::AccountId;
use shared_kernel::errors::{DomainError, Result};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::domain::account::entities::AccountMetadata;
use crate::domain::repositories::AccountMetadataRepository;

/// Stub pour les tests unitaires.
/// Utilise des Mutex internes pour permettre l'usage dans des tests async (Tokio).
#[derive(Default)]
pub struct AccountMetadataRepositoryStub {
    metadata_map: Arc<Mutex<HashMap<AccountId, AccountMetadata>>>,
    error_to_return: Arc<Mutex<Option<DomainError>>>,
}

impl AccountMetadataRepositoryStub {
    pub fn new() -> Self {
        Self::default()
    }

    // --- Helpers pour l'Arrange (Préparation des tests) ---

    /// Injecte des données initiales dans le stub.
    pub fn add_metadata(&self, metadata: AccountMetadata) {
        let mut map = self.metadata_map.lock().expect("Lock failed");
        map.insert(metadata.account_id().clone(), metadata);
    }

    /// Simule une erreur imminente lors du prochain appel au repository.
    pub fn set_error(&self, err: DomainError) {
        let mut error_slot = self.error_to_return.lock().expect("Lock failed");
        *error_slot = Some(err);
    }

    /// Réinitialise l'état du stub.
    pub fn clear(&self) {
        self.metadata_map.lock().expect("Lock failed").clear();
        *self.error_to_return.lock().expect("Lock failed") = None;
    }

    // --- Helpers pour l'Assert (Vérification des résultats) ---

    /// Récupère une entité directement (pour vérification post-exécution).
    pub fn find_by_id(&self, id: &AccountId) -> Option<AccountMetadata> {
        self.metadata_map.lock().expect("Lock failed").get(id).cloned()
    }

    /// Vérifie si une entité existe.
    pub fn exists(&self, id: &AccountId) -> bool {
        self.metadata_map.lock().expect("Lock failed").contains_key(id)
    }

    /// Retourne le nombre d'entrées en "base".
    pub fn count(&self) -> usize {
        self.metadata_map.lock().expect("Lock failed").len()
    }

    // --- Logique interne ---

    fn check_error(&self) -> Result<()> {
        if let Some(err) = self.error_to_return.lock().expect("Lock failed").clone() {
            return Err(err);
        }
        Ok(())
    }
}

#[async_trait]
impl AccountMetadataRepository for AccountMetadataRepositoryStub {
    async fn fetch_by_account_id(
        &self, 
        id: &AccountId, 
        _tx: Option<&mut dyn Transaction> 
    ) -> Result<Option<AccountMetadata>> {
        self.check_error()?;
        
        Ok(self.find_by_id(id))
    }

    async fn save(
        &self,
        metadata: &AccountMetadata,
        original: Option<&AccountMetadata>,
        _tx: Option<&mut dyn Transaction>,
    ) -> Result<()> {
        self.check_error()?;

        let mut map = self.metadata_map.lock().expect("Lock failed");
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