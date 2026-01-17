// backend/services/account/outbox_processor/src/main.rs

use shared_kernel::errors::AppResult;
use shared_kernel::infrastructure::bootstrap::run_outbox_relay;


#[tokio::main]
async fn main() -> AppResult<()> {
    run_outbox_relay(
        "Account",
        "account.events",
    ).await
}