use async_trait::async_trait;
use shared_kernel::{
    core::{Error, Result, Transaction, Versioned},
    messaging::{Event, EventEmitter},
    types::{AccountId, Email, Phone, Region, SubId},
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use account::entities::Account;
use account::repositories::AccountRepository;

#[derive(Default)]
pub struct AccountRepositoryStub {
    accounts: Arc<Mutex<HashMap<AccountId, Account>>>,
    error_to_return: Arc<Mutex<Option<Error>>>,
    captured_events: Mutex<HashMap<AccountId, Vec<Box<dyn Event>>>>,
}

impl AccountRepositoryStub {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&self, account: Account) {
        let mut map = self.accounts.lock().unwrap();
        map.insert(account.account_id(), account);
    }

    pub fn set_error(&self, err: Error) {
        let mut slot = self.error_to_return.lock().unwrap();
        *slot = Some(err);
    }

    pub fn set_error_once(&self, err: Error) {
        let mut slot = self.error_to_return.lock().unwrap();
        *slot = Some(err);
    }

    pub fn find_direct(&self, id: AccountId) -> Option<Account> {
        self.accounts.lock().unwrap().get(&id).cloned()
    }

    fn check_error(&self) -> Result<()> {
        let mut slot = self.error_to_return.lock().unwrap();
        if let Some(err) = slot.take() {
            return Err(err);
        }
        Ok(())
    }

    pub async fn get_captured_events(&self, account_id: AccountId) -> Vec<Box<dyn Event>> {
        let captured = self.captured_events.lock().unwrap();
        captured.get(&account_id).cloned().unwrap_or_default()
    }
}

#[async_trait]
impl AccountRepository for AccountRepositoryStub {
    async fn find_by_id(
        &self,
        _region: Region,
        id: AccountId,
        _tx: Option<&mut dyn Transaction>,
    ) -> Result<Option<Account>> {
        self.check_error()?;
        Ok(self.find_direct(id))
    }

    async fn find_by_sub_id(
        &self,
        _region: Region,
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

    async fn find_id_by_email(
        &self,
        _region: Region,
        email: &Email,
        _tx: Option<&mut dyn Transaction>,
    ) -> Result<Option<AccountId>> {
        self.check_error()?;
        let map = self.accounts.lock().unwrap();
        Ok(map
            .values()
            .find(|a| a.identity().email() == Some(email))
            .map(|a| a.identity().account_id()))
    }

    async fn find_id_by_sub_id(
        &self,
        _region: Region,
        ext_id: &SubId,
        _tx: Option<&mut dyn Transaction>,
    ) -> Result<Option<AccountId>> {
        self.check_error()?;
        let map = self.accounts.lock().unwrap();
        Ok(map
            .values()
            .find(|a| a.identity().sub_id() == Some(ext_id))
            .map(|a| a.identity().account_id()))
    }

    // --- VÉRIFICATIONS D'UNICITÉ ---

    async fn exists_by_email(
        &self,
        _region: Region,
        email: &Email,
        _tx: Option<&mut dyn Transaction>,
    ) -> Result<bool> {
        self.check_error()?;
        let map = self.accounts.lock().unwrap();
        Ok(map.values().any(|a| a.identity().email() == Some(email)))
    }

    async fn exists_by_phone(
        &self,
        _region: Region,
        phone: &Phone,
        _tx: Option<&mut dyn Transaction>,
    ) -> Result<bool> {
        self.check_error()?;
        let map = self.accounts.lock().unwrap();
        Ok(map.values().any(|a| a.identity().phone() == Some(phone)))
    }

    async fn exists_by_sub_id(
        &self,
        _region: Region,
        ext_id: &SubId,
        _tx: Option<&mut dyn Transaction>,
    ) -> Result<bool> {
        self.check_error()?;
        let map = self.accounts.lock().unwrap();
        Ok(map.values().any(|a| a.identity().sub_id() == Some(ext_id)))
    }

    // --- ÉCRITURES ---

    async fn save(
        &self,
        _region: Region,
        account: &mut Account,
        _tx: Option<&mut dyn Transaction>,
    ) -> Result<()> {
        self.check_error()?;

        let events = account.pull_events();
        if !events.is_empty() {
            let mut captured = self.captured_events.lock().unwrap();
            captured
                .entry(account.account_id())
                .or_default()
                .extend(events);
        }

        let mut map = self.accounts.lock().unwrap();
        let new_id = account.account_id();
        let new_version = account.version();

        let old_id_opt = map
            .iter()
            .find(|(_id, a)| {
                a.identity().account_id().uuid() == new_id.uuid()
                    && a.identity().account_id() != new_id
            })
            .map(|(id, _)| *id);

        if let Some(old_id) = old_id_opt {
            map.remove(&old_id);
        }

        let existing_opt = map.get(&new_id);

        match existing_opt {
            None => {
                if new_version > 1 && old_id_opt.is_none() {
                    return Err(Error::concurrency_conflict(format!(
                        "Cannot insert new account with version {}",
                        new_version
                    )));
                }
                if let Some(new_sub) = account.identity().sub_id() {
                    if map.values().any(|a| a.identity().sub_id() == Some(new_sub)) {
                        return Err(Error::already_exists(
                            "Account",
                            "sub_id",
                            new_sub.to_string(),
                        ));
                    }
                }
            }
            Some(existing) => {
                let current_version = existing.version();

                if new_version == current_version {
                    return Ok(());
                }

                if new_version < current_version || new_version > current_version + 1 {
                    return Err(Error::concurrency_conflict(format!(
                        "OCC mismatch: v{} -> v{}",
                        current_version, new_version
                    )));
                }

                if let Some(new_sub) = account.identity().sub_id() {
                    let duplicate_exists = map.values().any(|a| {
                        a.identity().account_id() != new_id
                            && a.identity().sub_id() == Some(new_sub)
                    });

                    if duplicate_exists {
                        return Err(Error::already_exists(
                            "Account",
                            "sub_id",
                            new_sub.to_string(),
                        ));
                    }
                }
            }
        }

        map.insert(new_id, account.clone());
        Ok(())
    }

    async fn create(
        &self,
        region: Region,
        account: &Account,
        _tx: &mut dyn Transaction,
    ) -> Result<()> {
        let mut acc = account.clone();
        self.save(region, &mut acc, None).await
    }

    async fn delete(
        &self,
        _region: Region,
        id: AccountId,
        _tx: &mut dyn Transaction,
    ) -> Result<()> {
        self.check_error()?;
        let mut map = self.accounts.lock().unwrap();
        if map.remove(&id).is_none() {
            return Err(Error::not_found("Account", id.to_string()));
        }
        Ok(())
    }
}
