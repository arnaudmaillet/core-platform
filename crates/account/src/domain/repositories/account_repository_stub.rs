// crates/account/src/domain/repositories/stubs/account_repository_stub.rs

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use async_trait::async_trait;
use shared_kernel::domain::Identifier;
use shared_kernel::domain::events::AggregateRoot;
use shared_kernel::domain::transaction::Transaction;
use shared_kernel::domain::value_objects::AccountId;
use shared_kernel::errors::{DomainError, Result};

use crate::domain::account::entities::Account;
use crate::domain::repositories::AccountRepository;
use crate::domain::value_objects::{AccountState, Email, ExternalId, PhoneNumber};

#[derive(Default)]
pub struct AccountRepositoryStub {
    /// Stockage en mémoire : AccountId -> Account
    pub accounts_map: Arc<Mutex<HashMap<AccountId, Account>>>,
    /// Simulation d'erreur forcée
    pub error_to_return: Arc<Mutex<Option<DomainError>>>,
}

impl AccountRepositoryStub {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_account(&self, account: Account) {
        self.accounts_map.lock().unwrap().insert(account.id().clone(), account);
    }

    fn check_error(&self) -> Result<()> {
        if let Some(err) = self.error_to_return.lock().unwrap().clone() {
            return Err(err);
        }
        Ok(())
    }
}

#[async_trait]
impl AccountRepository for AccountRepositoryStub {
    // --- RÉSOLUTIONS & LECTURES ---

    async fn fetch_by_id(&self, id: &AccountId, _tx: Option<&mut dyn Transaction>) -> Result<Option<Account>> {
        self.check_error()?;
        Ok(self.accounts_map.lock().unwrap().get(id).cloned())
    }

    async fn resolve_id_from_external_id(&self, ext_id: &ExternalId) -> Result<Option<AccountId>> {
        self.check_error()?;
        Ok(self.accounts_map.lock().unwrap().values()
            .find(|a| a.external_id() == ext_id)
            .map(|a| a.id().clone()))
    }

    async fn resolve_id_from_email(&self, email: &Email) -> Result<Option<AccountId>> {
        self.check_error()?;
        Ok(self.accounts_map.lock().unwrap().values()
            .find(|a| a.email() == email)
            .map(|a| a.id().clone()))
    }

    // --- VÉRIFICATIONS D'EXISTENCE ---

    async fn exists_by_external_id(&self, ext_id: &ExternalId) -> Result<bool> {
        self.check_error()?;
        Ok(self.accounts_map.lock().unwrap().values().any(|a| a.external_id() == ext_id))
    }
    
    async fn exists_by_email(&self, email: &Email) -> Result<bool> {
        self.check_error()?;
        Ok(self.accounts_map.lock().unwrap().values().any(|a| a.email() == email))
    }

    async fn exists_by_phone(&self, phone: &PhoneNumber) -> Result<bool> {
        self.check_error()?;
        Ok(self.accounts_map.lock().unwrap().values().any(|a| a.phone_number() == Some(phone)))
    }

    // --- MUTATIONS DE L'ÉTAT (COMMANDES) ---

    async fn save(&self, account: &Account, original: Option<&Account>, _tx: Option<&mut dyn Transaction>) -> Result<()> {
        self.check_error()?;
        let mut map = self.accounts_map.lock().unwrap();
        
        // Simulation du verrouillage optimiste
        if let Some(orig) = original {
            let current = map.get(account.id()).ok_or_else(|| DomainError::NotFound {
                entity: "Account",
                id: account.id().as_string(),
            })?;

            if current.version() != orig.version() {
                return Err(DomainError::ConcurrencyConflict {
                    reason: format!("Account {}: version mismatch in stub", account.id().as_string())
                });
            }
        }

        map.insert(account.id().clone(), account.clone());
        Ok(())
    }

    async fn transit_to_state(&self, id: &AccountId, state: AccountState, _tx: &mut dyn Transaction) -> Result<()> {
        self.check_error()?;
        let mut map = self.accounts_map.lock().unwrap();
        if let Some(acc) = map.get_mut(id) {
            // Dans un stub, on simule l'effet de transit_to_state
            // Note: En prod, cette méthode est souvent une optimisation SQL directe.
            // Ici, on pourrait charger, modifier l'état, incrémenter la version.
            Ok(()) 
        } else {
            Err(DomainError::NotFound { entity: "Account", id: id.as_string() })
        }
    }

    // --- OPÉRATIONS DE MAINTENANCE ---

    async fn record_activity(&self, _id: &AccountId) -> Result<()> {
        self.check_error()?;
        Ok(())
    }

    async fn hard_delete(&self, id: &AccountId, _tx: &mut dyn Transaction) -> Result<()> {
        self.check_error()?;
        if self.accounts_map.lock().unwrap().remove(id).is_none() {
            return Err(DomainError::NotFound { entity: "Account", id: id.as_string() });
        }
        Ok(())
    }
}