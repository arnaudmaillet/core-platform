// crates/account/src/domain/repositories/stubs/account_identity_repository_stub.rs

use async_trait::async_trait;
use shared_kernel::domain::events::AggregateRoot;
use shared_kernel::domain::transaction::Transaction;
use shared_kernel::domain::value_objects::AccountId;
use shared_kernel::errors::{DomainError, Result};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::domain::account::entities::AccountIdentity;
use crate::domain::repositories::AccountIdentityRepository;
use crate::domain::value_objects::{AccountState, Email, ExternalId, PhoneNumber};

#[derive(Default)]
pub struct AccountIdentityRepositoryStub {
    identity_map: Arc<Mutex<HashMap<AccountId, AccountIdentity>>>,
    error_to_return: Arc<Mutex<Option<DomainError>>>,
}

impl AccountIdentityRepositoryStub {
    pub fn new() -> Self {
        Self::default()
    }

    // --- Helpers pour l'Arrange (Setup) ---

    pub fn insert(&self, identity: AccountIdentity) {
        let mut map = self.identity_map.lock().expect("Lock failed");
        map.insert(identity.account_id().clone(), identity);
    }

    pub fn set_error(&self, err: DomainError) {
        let mut error_slot = self.error_to_return.lock().expect("Lock failed");
        *error_slot = Some(err);
    }

    // --- Helpers pour l'Assert (Vérification) ---

    pub fn find_by_id(&self, id: &AccountId) -> Option<AccountIdentity> {
        self.identity_map.lock().expect("Lock failed").get(id).cloned()
    }

    pub fn count(&self) -> usize {
        self.identity_map.lock().expect("Lock failed").len()
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
impl AccountIdentityRepository for AccountIdentityRepositoryStub {
    async fn fetch_by_account_id(
        &self,
        account_id: &AccountId,
        _tx: Option<&mut dyn Transaction>,
    ) -> Result<Option<AccountIdentity>> {
        self.check_error()?;
        Ok(self.find_by_id(account_id))
    }

    async fn resolve_id_from_external_id(&self, ext_id: &ExternalId) -> Result<Option<AccountId>> {
        self.check_error()?;
        let map = self.identity_map.lock().expect("Lock failed");
        Ok(map.values()
            .find(|a| a.external_id() == ext_id)
            .map(|a| a.account_id().clone()))
    }

    async fn resolve_id_from_email(&self, email: &Email) -> Result<Option<AccountId>> {
        self.check_error()?;
        let map = self.identity_map.lock().expect("Lock failed");
        Ok(map.values()
            .find(|a| a.email() == email)
            .map(|a| a.account_id().clone()))
    }

    async fn exists_by_external_id(&self, ext_id: &ExternalId) -> Result<bool> {
        self.check_error()?;
        let map = self.identity_map.lock().expect("Lock failed");
        Ok(map.values().any(|a| a.external_id() == ext_id))
    }

    async fn exists_by_email(&self, email: &Email) -> Result<bool> {
        self.check_error()?;
        let map = self.identity_map.lock().expect("Lock failed");
        Ok(map.values().any(|a| a.email() == email))
    }

    async fn exists_by_phone(&self, phone: &PhoneNumber) -> Result<bool> {
        self.check_error()?;
        let map = self.identity_map.lock().expect("Lock failed");
        Ok(map.values().any(|a| a.phone_number() == Some(phone)))
    }

    async fn save(
        &self,
        identity: &AccountIdentity,
        original: Option<&AccountIdentity>,
        _tx: Option<&mut dyn Transaction>,
    ) -> Result<()> {
        self.check_error()?;
        let mut map = self.identity_map.lock().expect("Lock failed");
        let account_id = identity.account_id();

        match original {
            Some(orig) => {
                let current = map.get(account_id).ok_or_else(|| DomainError::NotFound {
                    entity: "AccountIdentity",
                    id: account_id.to_string(),
                })?;

                if current.version() != orig.version() {
                    return Err(DomainError::ConcurrencyConflict {
                        reason: format!("OCC Conflict: Stub v{}, Input v{}", current.version(), orig.version()),
                    });
                }
            }
            None => {
                if map.contains_key(account_id) {
                    return Err(DomainError::AlreadyExists {
                        entity: "AccountIdentity",
                        field: "id",
                        value: account_id.to_string(),
                    });
                }
            }
        }

        map.insert(account_id.clone(), identity.clone());
        Ok(())
    }

    async fn transit_to_state(
        &self,
        account_id: &AccountId,
        _state: AccountState,
        _tx: &mut dyn Transaction,
    ) -> Result<()> {
        self.check_error()?;
        let mut map = self.identity_map.lock().expect("Lock failed");
        if let Some(acc) = map.get_mut(account_id) {
            // Simulation simple pour le stub
            // Note : Pour être parfait, il faudrait une méthode interne sur AccountIdentity 
            // pour forcer le changement d'état sans passer par la logique métier complexe
            // Mais pour l'instant, on se contente de valider que l'ID existe.
            Ok(())
        } else {
            Err(DomainError::NotFound {
                entity: "AccountIdentity",
                id: account_id.to_string(),
            })
        }
    }

    async fn record_activity(&self, _id: &AccountId) -> Result<()> {
        self.check_error()?;
        Ok(())
    }

    async fn hard_delete(&self, account_id: &AccountId, _tx: &mut dyn Transaction) -> Result<()> {
        self.check_error()?;
        let mut map = self.identity_map.lock().expect("Lock failed");
        if map.remove(account_id).is_none() {
            return Err(DomainError::NotFound {
                entity: "AccountIdentity",
                id: account_id.to_string(),
            });
        }
        Ok(())
    }
}