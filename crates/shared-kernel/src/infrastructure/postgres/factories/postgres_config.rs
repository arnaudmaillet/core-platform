// shared-kernel/src/infrastructure/postgres/postgres_factory.rs

use crate::errors::{AppError, AppResult, ErrorCode};
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use std::time::Duration;

pub struct PostgresConfig {
    pub url: String,
    pub max_connections: u32,
    pub min_connections: u32,
    pub connect_timeout: Duration,
}

impl PostgresConfig {
    pub fn from_env() -> AppResult<Self> {
        // La DATABASE_URL reste obligatoire
        let url = std::env::var("PROFILE_DB_URL")
            .map_err(|_| AppError::new(ErrorCode::InternalError, "PROFILE_DB_URL must be set"))?;

        // Pour les chiffres, on tente de parser, sinon on met une valeur par défaut
        let max_connections = std::env::var("PROFILE_DB_MAX_CONNECTIONS")
            .unwrap_or_else(|_| "10".to_string())
            .parse::<u32>()
            .map_err(|_| AppError::new(ErrorCode::InternalError, "Invalid PROFILE_DB_MAX_CONNECTIONS"))?;

        let min_connections = std::env::var("PROFILE_DB_MIN_CONNECTIONS")
            .unwrap_or_else(|_| "2".to_string())
            .parse::<u32>()
            .map_err(|_| AppError::new(ErrorCode::InternalError, "Invalid PROFILE_DB_MIN_CONNECTIONS"))?;

        let timeout_secs = std::env::var("PROFILE_DB_CONNECT_TIMEOUT")
            .unwrap_or_else(|_| "3".to_string())
            .parse::<u64>()
            .map_err(|_| AppError::new(ErrorCode::InternalError, "Invalid PROFILE_DB_CONNECT_TIMEOUT"))?;

        Ok(Self {
            url,
            max_connections,
            min_connections,
            connect_timeout: Duration::from_secs(timeout_secs),
        })
    }
}

pub async fn create_postgres_pool(config: &PostgresConfig) -> AppResult<PgPool> {
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
