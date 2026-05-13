// crates/shared-kernel/src/infrastructure/postgres/utils/mod.rs

mod migrations;
pub use migrations::run_kernel_postgres_migrations;