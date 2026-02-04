// crates/profile/tests/common/mod.rs

mod setup_postgres_test_db;
mod setup_scylla_test_db;

pub use setup_postgres_test_db::setup_postgres_test_db;
pub use setup_scylla_test_db::setup_scylla_db;