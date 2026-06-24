use crate::config::{PostgresConfig, ShardedPostgresConfig, TopologyConfig};
use crate::error::StorageError;
use crate::routing::{ShardCluster, ShardId};
use crate::transaction::manager::TransactionManager;
use futures::future::join_all;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::ConnectOptions as _;
use sqlx::PgPool;
use std::collections::HashMap;
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

/// Constructs a [`ShardCluster`] by concurrently connecting all shard pools.
///
/// All shard connections are attempted in parallel. Construction **fails fast**
/// if any shard pool fails to connect — a partial cluster is not a safe runtime
/// state because requests would silently route to a missing shard.
pub struct PgClusterBuilder;

impl PgClusterBuilder {
    /// Validates the shard configuration, opens all shard pools concurrently,
    /// and assembles them into a [`ShardCluster`].
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::Configuration`] if `shard_urls.len()` does not
    /// equal `shard_count`, or if any individual pool fails to connect.
    #[tracing::instrument(name = "postgres.cluster.build", skip(config), fields(
        shard_count = config.shard_count,
    ))]
    pub async fn build(config: ShardedPostgresConfig) -> Result<ShardCluster, StorageError> {
        if config.shard_urls.len() != usize::from(config.shard_count) {
            return Err(StorageError::Configuration {
                message: format!(
                    "shard_urls has {} entries but shard_count is {}; they must be equal",
                    config.shard_urls.len(),
                    config.shard_count,
                ),
            });
        }

        // Extract Copy fields before consuming shard_urls.
        let shard_count       = config.shard_count;
        let max_connections   = config.max_connections;
        let min_connections   = config.min_connections;
        let acquire_timeout   = config.acquire_timeout;
        let idle_timeout      = config.idle_timeout;
        let max_lifetime      = config.max_lifetime;
        let statement_log_level     = config.statement_log_level;
        let slow_statement_threshold = config.slow_statement_threshold;

        let pool_futures = config.shard_urls.into_iter().enumerate().map(|(i, url)| {
            let pool_config = PostgresConfig {
                database_url: url,
                max_connections,
                min_connections,
                acquire_timeout,
                idle_timeout,
                max_lifetime,
                statement_log_level,
                slow_statement_threshold,
            };
            async move {
                let pool = PgPoolBuilder::build(pool_config).await?;
                Ok::<_, StorageError>((ShardId(i as u16), pool))
            }
        });

        let results = join_all(pool_futures).await;

        let mut shards = HashMap::with_capacity(usize::from(shard_count));
        for result in results {
            let (shard_id, pool) = result?;
            shards.insert(shard_id, pool);
        }

        Ok(ShardCluster::new(shards, shard_count))
    }
}

/// Primary service bootstrap entry point for topology-agnostic initialization.
///
/// Reads a [`TopologyConfig`] and dispatches internally to either
/// [`PgPoolBuilder`] (SingleNode) or [`PgClusterBuilder`] (ApplicationSharded),
/// returning a fully configured [`TransactionManager`] in both cases.
///
/// # Usage pattern
///
/// ```rust,ignore
/// let config = TopologyConfig::from_env();
/// let tx_manager = TopologyBuilder::build(config).await?;
/// // tx_manager is topology-agnostic — use run_on_shard() in all service code.
/// ```
pub struct TopologyBuilder;

impl TopologyBuilder {
    #[tracing::instrument(name = "postgres.topology.build", skip(config))]
    pub async fn build(config: TopologyConfig) -> Result<TransactionManager, StorageError> {
        match config {
            TopologyConfig::SingleNode(pg_config) => {
                let pool = PgPoolBuilder::build(pg_config).await?;
                Ok(TransactionManager::new(pool))
            }
            TopologyConfig::ApplicationSharded(shard_config) => {
                let cluster = PgClusterBuilder::build(shard_config).await?;
                Ok(TransactionManager::from_cluster(cluster))
            }
        }
    }
}
