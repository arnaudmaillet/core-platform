// backend/services/profile/outbox_processor/src/main.rs

use shared_kernel::core::Result;
use shared_kernel::messaging::run_outbox_relay;

#[tokio::main]
async fn main() -> Result<()> {
    run_outbox_relay("Profile", "profile.events").await
}
