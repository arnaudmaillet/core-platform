// crates/shared-kernel/src/infrastructure/scylla/factories/scylla_factory.rs

use std::time::Duration;

#[derive(Debug, Clone)]
pub struct ScyllaConfig {
    pub connect_timeout: Duration,
}