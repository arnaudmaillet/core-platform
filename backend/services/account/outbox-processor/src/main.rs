// backend/services/account/outbox_processor/src/main.rs

use shared_kernel::core::Result;
use shared_kernel::messaging::run_outbox_relay;

#[tokio::main]
async fn main() -> Result<()> {
    run_outbox_relay("Account", "account.events.v1").await
}
