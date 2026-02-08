// crates/profile/tests/common/mod.rs

mod setup_infrastructure;

pub use setup_infrastructure:: {setup_postgres_test_db, setup_scylla_db, setup_redis_test_cache};
