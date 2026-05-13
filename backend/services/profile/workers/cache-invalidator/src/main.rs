// backend/services/profile/cache_invalidator/src/main.rs

use shared_kernel::core::Result;
use shared_kernel::cache::run_cache_worker;

#[tokio::main]
async fn main() -> Result<()> {
    run_cache_worker("Account", "profile.events", "profile-cache-group").await
}
