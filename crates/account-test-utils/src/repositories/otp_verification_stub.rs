// crates/account/src/application/use_cases/access_management/test_utils/otp_repository_stub.rs
// (ou dans ton crate partagé 'account_test_utils')

use account::repositories::OtpRepository;
use async_trait::async_trait;
use shared_kernel::core::Result;
use shared_kernel::types::AccountId;
use std::collections::HashMap;
use std::sync::RwLock;

#[derive(Default)]
pub struct OtpRepositoryStub {
    store: RwLock<HashMap<String, String>>,
}

impl OtpRepositoryStub {
    pub fn new() -> Self {
        Self {
            store: RwLock::new(HashMap::new()),
        }
    }

    /// Helper exclusif aux tests pour pré-remplir le faux cache Redis (Phase Arrange)
    pub fn seed_code(&self, account_id: AccountId, purpose: &str, code: &str) {
        let mut store = self.store.write().unwrap();
        let key = self.format_key(&account_id, purpose);
        store.insert(key, code.to_string());
    }

    fn format_key(&self, account_id: &AccountId, purpose: &str) -> String {
        format!("{}:{}", account_id.to_string(), purpose)
    }
}

#[async_trait]
impl OtpRepository for OtpRepositoryStub {
    async fn store_code(&self, account_id: &AccountId, purpose: &str, code: &str) -> Result<()> {
        let mut store = self.store.write().unwrap();
        let key = self.format_key(account_id, purpose);
        store.insert(key, code.to_string());
        Ok(())
    }

    async fn get_code(&self, account_id: &AccountId, purpose: &str) -> Result<Option<String>> {
        let store = self.store.read().unwrap();
        let key = self.format_key(account_id, purpose);
        Ok(store.get(&key).cloned())
    }

    async fn invalidate(&self, account_id: &AccountId, purpose: &str) -> Result<()> {
        let mut store = self.store.write().unwrap();
        let key = self.format_key(account_id, purpose);
        store.remove(&key);
        Ok(())
    }
}
