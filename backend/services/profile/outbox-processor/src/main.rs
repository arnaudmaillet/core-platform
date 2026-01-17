// backend/services/profile/outbox_processor/src/main.rs

use shared_kernel::errors::AppResult;
use shared_kernel::infrastructure::bootstrap::run_outbox_relay;


#[tokio::main]
async fn main() -> AppResult<()> {
    run_outbox_relay(
        "Profile",
        "profile.events",
    ).await
}