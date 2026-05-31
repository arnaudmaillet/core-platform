// crates/account/src/infrastructure/cache/redis_otp_repository.rs

use crate::domain::repositories::OtpRepository;
use async_trait::async_trait;
use shared_kernel::cache::CacheRepository;
use shared_kernel::core::Result;
use shared_kernel::types::AccountId;
use std::sync::Arc;
use std::time::Duration;

pub struct FredOtpRepository {
    cache: Arc<dyn CacheRepository>,
}

impl FredOtpRepository {
    pub fn new(cache: Arc<dyn CacheRepository>) -> Self {
        Self { cache }
    }

    fn format_key(&self, account_id: &AccountId, purpose: &str) -> String {
        format!("otp:account:{}:{}", account_id.to_string(), purpose)
    }
}

#[async_trait]
impl OtpRepository for FredOtpRepository {
    async fn store_code(&self, account_id: &AccountId, purpose: &str, code: &str) -> Result<()> {
        let key = self.format_key(account_id, purpose);
        let ttl = Duration::from_secs(15 * 60); // 15 minutes
        self.cache.set(&key, code, Some(ttl)).await
    }

    async fn get_code(&self, account_id: &AccountId, purpose: &str) -> Result<Option<String>> {
        let key = self.format_key(account_id, purpose);
        self.cache.get(&key).await
    }

    async fn invalidate(&self, account_id: &AccountId, purpose: &str) -> Result<()> {
        let key = self.format_key(account_id, purpose);
        self.cache.delete(&key).await
    }
}
