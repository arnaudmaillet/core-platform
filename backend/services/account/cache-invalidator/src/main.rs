// backend/services/account/cache_invalidator/src/main.rs

use shared_kernel::core::Result;
use shared_kernel::infrastructure::bootstrap::run_cache_worker;

#[tokio::main]
async fn main() -> Result<()> {
    run_cache_worker("Account", "account.events", "account-cache-group").await
}
