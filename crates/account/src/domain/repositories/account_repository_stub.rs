// crates/account/src/domain/repositories/stubs/account_repository_stub.rs

use async_trait::async_trait;
use shared_kernel::domain::events::AggregateRoot;
use shared_kernel::domain::transaction::Transaction;
use shared_kernel::domain::value_objects::{AccountId, Email, PhoneNumber, SubId};
use shared_kernel::errors::{DomainError, Result};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::domain::account::entities::Account;
use crate::domain::repositories::AccountRepository;

#[derive(Default)]
pub struct AccountRepositoryStub {
    accounts: Arc<Mutex<HashMap<AccountId, Account>>>,
    error_to_return: Arc<Mutex<Option<DomainError>>>,
}

impl AccountRepositoryStub {
    pub fn new() -> Self {
        Self::default()
    }

    // --- Helpers Setup (Arrange) ---

    pub fn insert(&self, account: Account) {
        let mut map = self.accounts.lock().unwrap();
        map.insert(account.identity().account_id().clone(), account);
    }

    pub fn set_error(&self, err: DomainError) {
        let mut slot = self.error_to_return.lock().unwrap();
        *slot = Some(err);
    }

    pub fn set_error_once(&self, err: DomainError) {
        let mut slot = self.error_to_return.lock().unwrap();
        *slot = Some(err);
    }

    // --- Helpers Assert ---

    pub fn find_direct(&self, id: &AccountId) -> Option<Account> {
        self.accounts.lock().unwrap().get(id).cloned()
    }

    // --- Logique Interne ---

    fn check_error(&self) -> Result<()> {
        let mut slot = self.error_to_return.lock().unwrap();
        if let Some(err) = slot.take() {
            return Err(err);
        }
        Ok(())
    }
}

#[async_trait]
impl AccountRepository for AccountRepositoryStub {
    async fn find_by_id(
        &self,
        id: &AccountId,
        _tx: Option<&mut dyn Transaction>,
    ) -> Result<Option<Account>> {
        self.check_error()?;
        Ok(self.find_direct(id))
    }

    async fn find_by_sub_id(
        &self,
        ext_id: &SubId,
        _tx: Option<&mut dyn Transaction>,
    ) -> Result<Option<Account>> {
        self.check_error()?;
        let map = self.accounts.lock().unwrap();
        let account = map
            .values()
            .find(|a| a.identity().sub_id() == Some(ext_id))
            .cloned();
        Ok(account)
    }

    // AJOUTÉ : _tx
    async fn find_id_by_email(
        &self,
        email: &Email,
        _tx: Option<&mut dyn Transaction>,
    ) -> Result<Option<AccountId>> {
        self.check_error()?;
        let map = self.accounts.lock().unwrap();
        Ok(map
            .values()
            .find(|a| a.identity().email() == Some(email))
            .map(|a| a.identity().account_id().clone()))
    }

    async fn find_id_by_sub_id(
        &self,
        ext_id: &SubId,
        _tx: Option<&mut dyn Transaction>, // Déjà présent ou à corriger
    ) -> Result<Option<AccountId>> {
        self.check_error()?;
        let map = self.accounts.lock().unwrap();
        Ok(map
            .values()
            .find(|a| a.identity().sub_id() == Some(ext_id))
            .map(|a| a.identity().account_id().clone()))
    }

    async fn exists_by_email(
        &self,
        email: &Email,
        _tx: Option<&mut dyn Transaction>,
    ) -> Result<bool> {
        self.check_error()?;
        let map = self.accounts.lock().unwrap();
        Ok(map.values().any(|a| a.identity().email() == Some(email)))
    }

    async fn exists_by_phone(
        &self,
        phone: &PhoneNumber,
        _tx: Option<&mut dyn Transaction>,
    ) -> Result<bool> {
        self.check_error()?;
        let map = self.accounts.lock().unwrap();
        Ok(map
            .values()
            .any(|a| a.identity().phone_number() == Some(phone)))
    }

    async fn exists_by_sub_id(
        &self,
        ext_id: &SubId,
        _tx: Option<&mut dyn Transaction>,
    ) -> Result<bool> {
        self.check_error()?;
        let map = self.accounts.lock().unwrap();
        Ok(map
            .values()
            .any(|a| a.identity().sub_id() == Some(ext_id)))
    }

    async fn create(&self, account: &Account, _tx: &mut dyn Transaction) -> Result<()> {
        self.check_error()?;
        let mut map = self.accounts.lock().unwrap();
        let id = account.identity().account_id().clone();

        if map.contains_key(&id) {
            return Err(DomainError::AlreadyExists {
                entity: "Account",
                field: "account_id",
                value: id.to_string(),
            });
        }

        map.insert(id, account.clone());
        Ok(())
    }

    async fn save(&self, account: &mut Account, _tx: Option<&mut dyn Transaction>) -> Result<()> {
        self.check_error()?;

        let mut map = self.accounts.lock().unwrap();
        let id = account.identity().account_id().clone();

        if let Some(existing) = map.get(&id) {
            let current_version = existing.metadata().version();
            let new_version = account.metadata().version();

            // 1. Logique OCC : On refuse les données obsolètes
            if new_version < current_version {
                return Err(DomainError::ConcurrencyConflict {
                    reason: format!(
                        "Stale data: Stub v{}, Input v{}",
                        current_version, new_version
                    ),
                });
            }
        }

        map.insert(id, account.clone());
        Ok(())
    }

    async fn delete(&self, id: &AccountId, _tx: &mut dyn Transaction) -> Result<()> {
        self.check_error()?;
        let mut map = self.accounts.lock().unwrap();
        if map.remove(id).is_none() {
            return Err(DomainError::NotFound {
                entity: "Account",
                id: id.to_string(),
            });
        }
        Ok(())
    }
}
