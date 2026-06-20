use crate::config::PostgresConfig;
use crate::error::StorageError;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::ConnectOptions as _;
use sqlx::PgPool;
use std::str::FromStr;

/// Constructs a [`PgPool`] from a [`PostgresConfig`], wiring sqlx statement
/// logging directly onto the process-global `tracing` subscriber installed by
/// the `telemetry` crate.
///
/// Every SQL statement executed through the returned pool emits a `tracing`
/// event at the configured level, and statements exceeding
/// [`PostgresConfig::slow_statement_threshold`] are additionally logged at
/// `WARN`. Both events carry the active distributed trace context, so they
/// appear inside the correct OTel span without any manual instrumentation.
pub struct PgPoolBuilder;

impl PgPoolBuilder {
    /// Validates the connection string, applies pool tuning options, registers
    /// the tracing hooks, and opens `min_connections` eagerly.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::Configuration`] if `database_url` cannot be
    /// parsed, or [`StorageError::Connection`] if the initial handshake fails.
    #[tracing::instrument(name = "postgres.pool.build", skip(config), fields(
        db.max_connections = config.max_connections,
        db.min_connections = config.min_connections,
    ))]
    pub async fn build(config: PostgresConfig) -> Result<PgPool, StorageError> {
        let connect_opts = PgConnectOptions::from_str(&config.database_url)
            .map_err(|e| StorageError::Configuration {
                message: format!("invalid DATABASE_URL: {e}"),
            })?
            .log_statements(config.statement_log_level.into())
            .log_slow_statements(log::LevelFilter::Warn, config.slow_statement_threshold);

        PgPoolOptions::new()
            .max_connections(config.max_connections)
            .min_connections(config.min_connections)
            .acquire_timeout(config.acquire_timeout)
            .idle_timeout(config.idle_timeout)
            .max_lifetime(config.max_lifetime)
            .connect_with(connect_opts)
            .await
            .map_err(StorageError::from)
    }
}
