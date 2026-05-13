// crates/shared-kernel/src/infrastructure/postgres/factories/postgres_context.rs

use crate::core::{Error, Result};
use crate::postgres::{PostgresConfig, PostgresContextBuilder};
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use std::time::Duration;

pub struct PostgresContext {
    pool: PgPool,
    url: String,
    max_connections: u32,
    min_connections: u32,
    connect_timeout: Duration,
}

impl PostgresContext {
    pub fn builder() -> Result<PostgresContextBuilder> {
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

    pub(crate) async fn restore(builder: PostgresContextBuilder) -> Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(builder.max_connections)
            .min_connections(builder.min_connections)
            .acquire_timeout(builder.connect_timeout)
            .connect(&builder.url)
            .await
            .map_err(|e| Error::internal(format!("Postgres Connection Failed: {}", e)))?;

        Ok(Self {
            pool,
            url: builder.url,
            max_connections: builder.max_connections,
            min_connections: builder.min_connections,
            connect_timeout: builder.connect_timeout,
        })
    }
}
