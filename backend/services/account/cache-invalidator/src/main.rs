// backend/services/account/cache_invalidator/src/main.rs

use shared_kernel::cache::run_cache_worker;
use shared_kernel::core::Result;

#[tokio::main]
async fn main() -> Result<()> {
    run_cache_worker("Account", "account.events", "account-cache-group").await
}
