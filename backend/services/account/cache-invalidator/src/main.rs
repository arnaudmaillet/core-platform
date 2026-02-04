// backend/services/account/cache_invalidator/src/main.rs

use shared_kernel::errors::AppResult;
use shared_kernel::infrastructure::bootstrap::run_cache_worker;

#[tokio::main]
async fn main() -> AppResult<()> {
    run_cache_worker("Account", "account.events", "account-cache-group").await
}
