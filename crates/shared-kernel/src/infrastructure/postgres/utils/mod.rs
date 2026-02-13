// crates/shared-kernel/src/infrastructure/postgres/utils/mod.rs

mod postgres_migrations;
pub use postgres_migrations::run_kernel_postgres_migrations;

#[cfg(feature = "test-utils")]
mod postgres_test_utils;


#[cfg(feature = "test-utils")]
pub use postgres_test_utils::setup_test_postgres;