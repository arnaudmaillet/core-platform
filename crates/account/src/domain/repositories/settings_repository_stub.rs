// crates/account/src/domain/repositories/stubs/account_settings_repository_stub.rs

use async_trait::async_trait;
use shared_kernel::domain::events::AggregateRoot;
use shared_kernel::domain::transaction::Transaction;
use shared_kernel::domain::value_objects::AccountId;
use shared_kernel::errors::{DomainError, Result};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::domain::account::entities::AccountSettings;
use crate::domain::repositories::AccountSettingsRepository;

#[derive(Default)]
pub struct AccountSettingsRepositoryStub {
    settings_map: Arc<Mutex<HashMap<AccountId, AccountSettings>>>,
    error_to_return: Arc<Mutex<Option<DomainError>>>,
}

impl AccountSettingsRepositoryStub {
    pub fn new() -> Self {
        Self::default()
    }

    // --- Helpers pour l'Arrange (Setup) ---

    pub fn insert(&self, settings: AccountSettings) {
        let mut map = self.settings_map.lock().expect("Lock failed");
        map.insert(settings.account_id().clone(), settings);
    }

    pub fn set_error(&self, err: DomainError) {
        let mut error_slot = self.error_to_return.lock().expect("Lock failed");
        *error_slot = Some(err);
    }

    // --- Helpers pour l'Assert (Vérification) ---

    pub fn find_by_id(&self, id: &AccountId) -> Option<AccountSettings> {
        self.settings_map
            .lock()
            .expect("Lock failed")
            .get(id)
            .cloned()
    }

    pub fn count(&self) -> usize {
        self.settings_map.lock().expect("Lock failed").len()
    }

    // --- Logique interne ---

    fn check_error(&self) -> Result<()> {
        if let Some(err) = self.error_to_return.lock().expect("Lock failed").clone() {
            return Err(err);
        }
        Ok(())
    }

    fn not_found(&self, id: &AccountId) -> DomainError {
        DomainError::NotFound {
            entity: "AccountSettings",
            id: id.to_string(),
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
        Ok(self.find_by_id(account_id))
    }

    async fn save(
        &self,
        settings: &AccountSettings,
        original: Option<&AccountSettings>,
        _tx: Option<&mut dyn Transaction>,
    ) -> Result<()> {
        self.check_error()?;
        let mut map = self.settings_map.lock().expect("Lock failed");
        let account_id = settings.account_id();

        match original {
            Some(orig) => {
                let current = map
                    .get(account_id)
                    .ok_or_else(|| self.not_found(account_id))?;

                if current.version() != orig.version() {
                    return Err(DomainError::ConcurrencyConflict {
                        reason: format!(
                            "OCC Conflict: Stub has v{}, but Input has v{}",
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
                        value: account_id.to_string(),
                    });
                }
            }
        }

        map.insert(account_id.clone(), settings.clone());
        Ok(())
    }
}
