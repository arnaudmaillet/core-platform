// crates/shared-kernel/src/infrastructure/postgres/factories/postgres_context.rs

use std::time::Duration;
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use crate::errors::{AppError, AppResult, ErrorCode};
use crate::infrastructure::postgres::factories::{PostgresConfig, PostgresContextBuilder};

pub struct PostgresContext {
    pool: PgPool,
    url: String,
    max_connections: u32,
    min_connections: u32,
    connect_timeout: Duration
}

impl PostgresContext {
    pub fn builder() -> AppResult<PostgresContextBuilder> {
        PostgresContextBuilder::new()
    }

    pub fn builder_raw() -> PostgresContextBuilder {
        PostgresContextBuilder::default()
    }

    pub fn pool(&self) -> PgPool {
        self.pool.clone()
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn config(&self) -> PostgresConfig {
        PostgresConfig {
            max_connections: self.max_connections,
            min_connections: self.min_connections,
            connect_timeout: self.connect_timeout,
        }
    }

    pub(crate) async fn restore(builder: PostgresContextBuilder) -> AppResult<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(builder.max_connections)
            .min_connections(builder.min_connections)
            .acquire_timeout(builder.connect_timeout)
            .connect(&builder.url)
            .await
            .map_err(|e| AppError::new(ErrorCode::InternalError, format!("Postgres Connection Failed: {}", e)))?;

        Ok(Self {
            pool,
            url: builder.url,
            max_connections: builder.max_connections,
            min_connections: builder.min_connections,
            connect_timeout: builder.connect_timeout,
        })
    }
}