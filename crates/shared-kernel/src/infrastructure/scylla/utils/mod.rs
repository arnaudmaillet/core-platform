// crates/shared-kernel/src/infrastructure/scylla/utils/mod.rs

#[cfg(feature = "test-utils")]
mod scylla_test_utils;

#[cfg(feature = "test-utils")]
pub use scylla_test_utils::setup_test_scylla;