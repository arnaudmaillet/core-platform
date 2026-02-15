// crates/shared-kernel/src/infrastructure/postgres/factories/postgres_builder.rs

use std::time::Duration;
use crate::errors::{AppError, AppResult, ErrorCode};
use crate::infrastructure::postgres::factories::PostgresContext;

pub struct PostgresContextBuilder {
    pub(crate) url: String,
    pub(crate) max_connections: u32,
    pub(crate) min_connections: u32,
    pub(crate) connect_timeout: Duration,
}


impl Default for PostgresContextBuilder {
    fn default() -> Self {
        Self {
            url: "postgres://postgres:postgres@localhost:5432/postgres".to_string(),
            max_connections: 5,
            min_connections: 2,
            connect_timeout: Duration::from_secs(3),
        }
    }
}

impl PostgresContextBuilder {
    pub fn new() -> AppResult<Self> {
        let url = std::env::var("PROFILE_DB_URL")
            .map_err(|_| AppError::new(ErrorCode::InternalError, "PROFILE_DB_URL must be set"))?;

        let max_connections = std::env::var("PROFILE_DB_MAX_CONNECTIONS")
            .ok().and_then(|v| v.parse().ok()).unwrap_or(10);

        let min_connections = std::env::var("PROFILE_DB_MIN_CONNECTIONS")
            .ok().and_then(|v| v.parse().ok()).unwrap_or(2);

        let timeout_secs = std::env::var("PROFILE_DB_CONNECT_TIMEOUT")
            .ok().and_then(|v| v.parse().ok()).unwrap_or(3);

        Ok(Self {
            url,
            max_connections,
            min_connections,
            connect_timeout: Duration::from_secs(timeout_secs),
        })
    }

    pub fn with_url(mut self, url: impl Into<String>) -> Self {
        self.url = url.into();
        self
    }

    pub fn with_max_connections(mut self, max: u32) -> Self {
        self.max_connections = max;
        self
    }

    pub fn with_min_connections(mut self, min: u32) -> Self {
        self.min_connections = min;
        self
    }

    pub fn with_timeout(mut self, duration: Duration) -> Self {
        self.connect_timeout = duration;
        self
    }

    pub async fn build(self) -> AppResult<PostgresContext> {
        PostgresContext::restore(self).await
    }
}