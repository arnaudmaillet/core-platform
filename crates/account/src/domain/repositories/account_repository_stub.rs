use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use async_trait::async_trait;
use shared_kernel::domain::Identifier;
use shared_kernel::domain::transaction::Transaction;
use shared_kernel::domain::value_objects::{AccountId, Username};
use shared_kernel::errors::{DomainError, Result};

use crate::domain::entities::Account;
use crate::domain::params::PatchUserParams;
use crate::domain::repositories::AccountRepository;
use crate::domain::value_objects::{AccountState, Email, ExternalId, PhoneNumber};

#[derive(Default)]
pub struct AccountRepositoryStub {
    /// Stockage en mémoire : AccountId -> Account
    pub accounts: Arc<Mutex<HashMap<AccountId, Account>>>,
    /// Permet de simuler une erreur spécifique retournée par n'importe quelle méthode
    pub error_to_return: Arc<Mutex<Option<DomainError>>>,
}

impl AccountRepositoryStub {
    pub fn new() -> Self {
        Self::default()
    }

    /// Helper pour injecter un compte manuellement avant un test
    pub fn add_account(&self, account: Account) {
        self.accounts.lock().unwrap().insert(account.id().clone(), account);
    }

    fn check_error(&self) -> Result<()> {
        if let Some(err) = self.error_to_return.lock().unwrap().clone() {
            return Err(err);
        }
        Ok(())
    }

    fn not_found(&self, id: String) -> DomainError {
        DomainError::NotFound {
            entity: "Account",
            id,
        }
    }
}

#[async_trait]
impl AccountRepository for AccountRepositoryStub {
    async fn find_account_id_by_email(&self, email: &Email) -> Result<Option<AccountId>> {
        self.check_error()?;
        Ok(self.accounts.lock().unwrap().values()
            .find(|a| a.email() == email)
            .map(|a| a.id().clone()))
    }

    async fn find_account_id_by_username(&self, username: &Username) -> Result<Option<AccountId>> {
        self.check_error()?;
        Ok(self.accounts.lock().unwrap().values()
            .find(|a| a.username() == username)
            .map(|a| a.id().clone()))
    }

    async fn find_account_id_by_external_id(
        &self,
        external_id: &ExternalId,
        _tx: Option<&mut dyn Transaction>,
    ) -> Result<Option<AccountId>> {
        self.check_error()?;
        Ok(self.accounts.lock().unwrap().values()
            .find(|a| a.external_id() == external_id)
            .map(|a| a.id().clone()))
    }

    async fn find_account_by_id(
        &self,
        id: &AccountId,
        _tx: Option<&mut dyn Transaction>,
    ) -> Result<Option<Account>> {
        self.check_error()?;
        Ok(self.accounts.lock().unwrap().get(id).cloned())
    }

    async fn exists_account_by_email(&self, email: &Email) -> Result<bool> {
        self.check_error()?;
        Ok(self.accounts.lock().unwrap().values().any(|a| a.email() == email))
    }

    async fn exists_account_by_username(&self, username: &Username) -> Result<bool> {
        self.check_error()?;
        Ok(self.accounts.lock().unwrap().values().any(|a| a.username() == username))
    }

    async fn exists_account_by_phone_number(&self, phone: &PhoneNumber) -> Result<bool> {
        self.check_error()?;
        Ok(self.accounts.lock().unwrap().values().any(|a| a.phone_number() == Some(phone)))
    }

    async fn create_account(&self, account: &Account, _tx: &mut dyn Transaction) -> Result<()> {
        self.check_error()?;
        self.accounts.lock().unwrap().insert(account.id().clone(), account.clone());
        Ok(())
    }

    async fn save(&self, user: &Account, _tx: Option<&mut dyn Transaction>) -> Result<()> {
        self.check_error()?;
        self.accounts.lock().unwrap().insert(user.id().clone(), user.clone());
        Ok(())
    }

    async fn patch_account_by_id(
        &self,
        id: &AccountId,
        _params: PatchUserParams,
        _tx: &mut dyn Transaction,
    ) -> Result<()> {
        self.check_error()?;

        if !self.accounts.lock().unwrap().contains_key(id) {
            return Err(self.not_found(id.as_string()));
        }

        Ok(())
    }


    async fn update_account_state_by_id(
        &self,
        id: &AccountId,
        state: AccountState,
        _tx: &mut dyn Transaction,
    ) -> Result<()> {
        self.check_error()?;
        if let Some(acc) = self.accounts.lock().unwrap().get_mut(id) {
            // Simulation simplifiée de l'update d'état
            // Dans un vrai Use Case, on préférera souvent charger l'entité et faire .save()
            Ok(())
        } else {
            Err(self.not_found(id.as_string()))
        }
    }

    async fn update_account_last_active(&self, _id: &AccountId) -> Result<()> {
        self.check_error()?;
        Ok(())
    }

    async fn delete(&self, id: &AccountId, _tx: &mut dyn Transaction) -> Result<()> {
        self.check_error()?;

        if self.accounts.lock().unwrap().remove(id).is_none() {
            return Err(self.not_found(id.as_string()));
        }

        Ok(())
    }
}