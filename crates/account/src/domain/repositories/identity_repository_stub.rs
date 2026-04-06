// crates/account/src/domain/repositories/stubs/account_repository_stub.rs

use async_trait::async_trait;
use shared_kernel::domain::Identifier;
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
    /// Stockage en mémoire : AccountId -> Account
    pub identity_map: Arc<Mutex<HashMap<AccountId, AccountIdentity>>>,
    /// Simulation d'erreur forcée
    pub error_to_return: Arc<Mutex<Option<DomainError>>>,
}

impl AccountIdentityRepositoryStub {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_account(&self, account: AccountIdentity) {
        self.identity_map
            .lock()
            .unwrap()
            .insert(account.id().clone(), account);
    }

    fn check_error(&self) -> Result<()> {
        if let Some(err) = self.error_to_return.lock().unwrap().clone() {
            return Err(err);
        }
        Ok(())
    }
}

#[async_trait]
impl AccountIdentityRepository for AccountIdentityRepositoryStub {
    // --- RÉSOLUTIONS & LECTURES ---

    async fn fetch_by_id(
        &self,
        id: &AccountId,
        _tx: Option<&mut dyn Transaction>,
    ) -> Result<Option<AccountIdentity>> {
        self.check_error()?;
        Ok(self.identity_map.lock().unwrap().get(id).cloned())
    }

    async fn resolve_id_from_external_id(&self, ext_id: &ExternalId) -> Result<Option<AccountId>> {
        self.check_error()?;
        Ok(self
            .identity_map
            .lock()
            .unwrap()
            .values()
            .find(|a| a.external_id() == ext_id)
            .map(|a| a.id().clone()))
    }

    async fn resolve_id_from_email(&self, email: &Email) -> Result<Option<AccountId>> {
        self.check_error()?;
        Ok(self
            .identity_map
            .lock()
            .unwrap()
            .values()
            .find(|a| a.email() == email)
            .map(|a| a.id().clone()))
    }

    // --- VÉRIFICATIONS D'EXISTENCE ---

    async fn exists_by_external_id(&self, ext_id: &ExternalId) -> Result<bool> {
        self.check_error()?;
        Ok(self
            .identity_map
            .lock()
            .unwrap()
            .values()
            .any(|a| a.external_id() == ext_id))
    }

    async fn exists_by_email(&self, email: &Email) -> Result<bool> {
        self.check_error()?;
        Ok(self
            .identity_map
            .lock()
            .unwrap()
            .values()
            .any(|a| a.email() == email))
    }

    async fn exists_by_phone(&self, phone: &PhoneNumber) -> Result<bool> {
        self.check_error()?;
        Ok(self
            .identity_map
            .lock()
            .unwrap()
            .values()
            .any(|a| a.phone_number() == Some(phone)))
    }

    // --- MUTATIONS DE L'ÉTAT (COMMANDES) ---

    async fn save(
        &self,
        identity: &AccountIdentity,
        original: Option<&AccountIdentity>,
        _tx: Option<&mut dyn Transaction>,
    ) -> Result<()> {
        self.check_error()?;
        let mut map = self.identity_map.lock().unwrap();

        // Simulation du verrouillage optimiste
        if let Some(orig) = original {
            let current = map.get(identity.id()).ok_or_else(|| DomainError::NotFound {
                entity: "Account",
                id: identity.id().as_string(),
            })?;

            if current.version() != orig.version() {
                return Err(DomainError::ConcurrencyConflict {
                    reason: format!(
                        "Account {}: version mismatch in stub",
                        identity.id().as_string()
                    ),
                });
            }
        }

        map.insert(identity.id().clone(), identity.clone());
        Ok(())
    }

    async fn transit_to_state(
        &self,
        id: &AccountId,
        state: AccountState,
        _tx: &mut dyn Transaction,
    ) -> Result<()> {
        self.check_error()?;
        let mut map = self.identity_map.lock().unwrap();
        if let Some(acc) = map.get_mut(id) {
            // Dans un stub, on simule l'effet de transit_to_state
            // Note: En prod, cette méthode est souvent une optimisation SQL directe.
            // Ici, on pourrait charger, modifier l'état, incrémenter la version.
            Ok(())
        } else {
            Err(DomainError::NotFound {
                entity: "Account",
                id: id.as_string(),
            })
        }
    }

    // --- OPÉRATIONS DE MAINTENANCE ---

    async fn record_activity(&self, _id: &AccountId) -> Result<()> {
        self.check_error()?;
        Ok(())
    }

    async fn hard_delete(&self, id: &AccountId, _tx: &mut dyn Transaction) -> Result<()> {
        self.check_error()?;
        if self.identity_map.lock().unwrap().remove(id).is_none() {
            return Err(DomainError::NotFound {
                entity: "Account",
                id: id.as_string(),
            });
        }
        Ok(())
    }
}
