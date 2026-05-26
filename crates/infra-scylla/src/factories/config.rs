// crates/infra-scylla/src/factories/config.rs

use std::time::Duration;

#[derive(Debug, Clone)]
pub struct ScyllaConfig {
    pub connect_timeout: Duration,
}