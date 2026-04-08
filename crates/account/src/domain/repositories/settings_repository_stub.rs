// crates/account/src/domain/repositories/stubs/account_settings_repository_stub.rs

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use async_trait::async_trait;
use shared_kernel::domain::Identifier;
use shared_kernel::domain::events::AggregateRoot;
use shared_kernel::domain::transaction::Transaction;
use shared_kernel::domain::value_objects::AccountId;
use shared_kernel::domain::value_objects::{PushToken, Timezone};
use shared_kernel::errors::{DomainError, Result};

use crate::domain::account::entities::AccountSettings;
use crate::domain::repositories::AccountSettingsRepository;

#[derive(Default)]
pub struct AccountSettingsRepositoryStub {
    pub settings_map: Arc<Mutex<HashMap<AccountId, AccountSettings>>>,
    pub error_to_return: Arc<Mutex<Option<DomainError>>>,
}

impl AccountSettingsRepositoryStub {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_settings(&self, settings: AccountSettings) {
        self.settings_map.lock().unwrap().insert(settings.account_id().clone(), settings);
    }

    fn check_error(&self) -> Result<()> {
        if let Some(err) = self.error_to_return.lock().unwrap().clone() {
            return Err(err);
        }
        Ok(())
    }

    fn not_found(&self, id: String) -> DomainError {
        DomainError::NotFound {
            entity: "AccountSettings",
            id,
        }
    }
}

#[async_trait]
impl AccountSettingsRepository for AccountSettingsRepositoryStub {
    async fn fetch_by_account_id(
        &self,
        account_id: &AccountId,
        _tx: Option<&mut dyn Transaction>,
    ) -> Result<Option<AccountSettings>> {
        self.check_error()?;
        Ok(self.settings_map.lock().unwrap().get(account_id).cloned())
    }

    async fn save(
        &self,
        settings: &AccountSettings,
        original: Option<&AccountSettings>,
        _tx: Option<&mut dyn Transaction>,
    ) -> Result<()> {
        self.check_error()?;
        let mut map = self.settings_map.lock().unwrap();
        let account_id = settings.account_id();

        match original {
            Some(orig) => {
                let current = map.get(account_id).ok_or_else(|| self.not_found(account_id.as_string()))?;
                
                if current.version() != orig.version() {
                    return Err(DomainError::ConcurrencyConflict {
                        reason: format!(
                            "AccountSettings OCC Conflict: Stub has v{}, but you provided v{}",
                            current.version(),
                            orig.version()
                        ),
                    });
                }
            }
            None => {
                if map.contains_key(account_id) {
                    return Err(DomainError::AlreadyExists {
                        entity: "AccountSettings",
                        field: "account_id",
                        value: account_id.as_string(),
                    });
                }
            }
        }

        map.insert(account_id.clone(), settings.clone());
        Ok(())
    }

    async fn update_timezone(
        &self,
        account_id: &AccountId,
        timezone: &Timezone,
        _tx: Option<&mut dyn Transaction>,
    ) -> Result<()> {
        self.check_error()?;
        let mut map = self.settings_map.lock().unwrap();

        if let Some(settings) = map.get_mut(account_id) {
            settings.update_timezone(timezone.clone())?;
            Ok(())
        } else {
            Err(self.not_found(account_id.as_string()))
        }
    }

    async fn add_push_token(
        &self,
        account_id: &AccountId,
        token: &PushToken,
        _tx: Option<&mut dyn Transaction>,
    ) -> Result<()> {
        self.check_error()?;
        let mut map = self.settings_map.lock().unwrap();

        if let Some(settings) = map.get_mut(account_id) {
            settings.add_push_token(token.clone())?;
            Ok(())
        } else {
            Err(self.not_found(account_id.as_string()))
        }
    }

    async fn remove_push_token(
        &self,
        account_id: &AccountId,
        token: &PushToken,
        _tx: Option<&mut dyn Transaction>,
    ) -> Result<()> {
        self.check_error()?;
        let mut map = self.settings_map.lock().unwrap();

        if let Some(settings) = map.get_mut(account_id) {
            settings.remove_push_token(token)?;
            Ok(())
        } else {
            Err(self.not_found(account_id.as_string()))
        }
    }
}