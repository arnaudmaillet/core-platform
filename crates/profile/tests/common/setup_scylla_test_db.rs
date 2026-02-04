// crates/profile/tests/common/setup_scylla_test_db.rs

use std::sync::Arc;
use scylla::client::session::Session;
use shared_kernel::infrastructure::scylla::utils::setup_test_scylla;

pub async fn setup_scylla_db() -> Arc<Session> {
    setup_test_scylla(
        &["./migrations/scylla"]
    ).await
}