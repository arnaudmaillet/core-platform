// shared-kernel/src/infrastructure/postgres/postgres_factory.rs
use std::time::Duration;

pub struct PostgresConfig {
    pub max_connections: u32,
    pub min_connections: u32,
    pub connect_timeout: Duration,
}

impl PostgresConfig {
    pub fn new(
        max_connections: u32,
        min_connections: u32,
        connect_timeout: Duration
    ) -> Self {
        Self {
            max_connections,
            min_connections,
            connect_timeout,
        }
    }
}