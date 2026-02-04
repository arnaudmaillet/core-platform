// shared-kernel/src/infrastructure/postgres/postgres_factory.rs

use crate::errors::{AppError, AppResult, ErrorCode};
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use std::time::Duration;

pub struct DbConfig {
    pub url: String,
    pub max_connections: u32,
    pub min_connections: u32,
    pub connect_timeout: Duration,
}

impl DbConfig {
    /// Charge la config depuis les variables d'environnement
    pub fn from_env() -> AppResult<Self> {
        Ok(Self {
            url: std::env::var("DATABASE_URL")
                .map_err(|_| AppError::new(ErrorCode::InternalError, "DATABASE_URL must be set"))?,
            max_connections: 20,
            min_connections: 5,
            connect_timeout: Duration::from_secs(3),
        })
    }
}

pub async fn create_postgres_pool(config: &DbConfig) -> AppResult<PgPool> {
    PgPoolOptions::new()
        .max_connections(config.max_connections)
        .min_connections(config.min_connections)
        .acquire_timeout(config.connect_timeout)
        .connect(&config.url)
        .await
        .map_err(|e| {
            AppError::new(
                ErrorCode::InternalError,
                format!("Failed to connect to Postgres: {}", e),
            )
        })
}
