// crates/account/src/domain/repositories/otp_repository.rs (ou dans ton module application)

use async_trait::async_trait;
use shared_kernel::core::Result;
use shared_kernel::types::AccountId;

#[async_trait]
pub trait OtpRepository: Send + Sync {
    async fn store_code(&self, account_id: &AccountId, purpose: &str, code: &str) -> Result<()>;
    async fn get_code(&self, account_id: &AccountId, purpose: &str) -> Result<Option<String>>;
    async fn invalidate(&self, account_id: &AccountId, purpose: &str) -> Result<()>;
}
