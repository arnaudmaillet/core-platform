use std::time::Duration;

/// Controls which SQL statements are emitted as `tracing` events by sqlx.
#[derive(Debug, Clone, Copy, Default)]
pub enum StatementLogLevel {
    Trace,
    #[default]
    Debug,
    Info,
    Warn,
    Error,
    Off,
}

impl From<StatementLogLevel> for log::LevelFilter {
    fn from(level: StatementLogLevel) -> Self {
        match level {
            StatementLogLevel::Trace => log::LevelFilter::Trace,
            StatementLogLevel::Debug => log::LevelFilter::Debug,
            StatementLogLevel::Info  => log::LevelFilter::Info,
            StatementLogLevel::Warn  => log::LevelFilter::Warn,
            StatementLogLevel::Error => log::LevelFilter::Error,
            StatementLogLevel::Off   => log::LevelFilter::Off,
        }
    }
}

/// Full configuration surface for a single PostgreSQL connection pool.
///
/// Construct via [`PostgresConfig::from_env`] for production, or build
/// the struct directly in tests.
#[derive(Debug, Clone)]
pub struct PostgresConfig {
    /// libpq-compatible connection string.
    /// `DATABASE_URL`
    pub database_url: String,

    /// Hard ceiling on open connections across the pool.
    /// `PG_MAX_CONNECTIONS` (default: 20)
    pub max_connections: u32,

    /// Connections kept alive even when idle; reduces cold-start latency.
    /// `PG_MIN_CONNECTIONS` (default: 2)
    pub min_connections: u32,

    /// Maximum time to wait for a free connection slot before returning
    /// [`StorageError::PoolTimedOut`].
    /// `PG_ACQUIRE_TIMEOUT_SECS` (default: 5)
    pub acquire_timeout: Duration,

    /// Idle connection reaping window. `None` disables idle-reaping.
    /// `PG_IDLE_TIMEOUT_SECS` (default: 600)
    pub idle_timeout: Option<Duration>,

    /// Maximum connection age. `None` disables max-lifetime recycling.
    /// `PG_MAX_LIFETIME_SECS` (default: 1800)
    pub max_lifetime: Option<Duration>,

    /// Log level for every individual SQL statement emitted to the active
    /// `tracing` subscriber. Set to `Off` in hot-path production services
    /// to reduce log volume.
    pub statement_log_level: StatementLogLevel,

    /// Statements taking longer than this threshold are logged at `WARN`.
    /// `PG_SLOW_STATEMENT_THRESHOLD_MS` (default: 1000)
    pub slow_statement_threshold: Duration,
}

impl PostgresConfig {
    /// Populates every field from environment variables, panicking on the
    /// only truly required variable (`DATABASE_URL`).
    pub fn from_env() -> Self {
        Self {
            database_url: std::env::var("DATABASE_URL")
                .expect("DATABASE_URL must be set"),

            max_connections: parse_env("PG_MAX_CONNECTIONS", 20),
            min_connections: parse_env("PG_MIN_CONNECTIONS", 2),

            acquire_timeout: Duration::from_secs(parse_env("PG_ACQUIRE_TIMEOUT_SECS", 5)),

            idle_timeout: parse_env_opt::<u64>("PG_IDLE_TIMEOUT_SECS")
                .map(Duration::from_secs)
                .or(Some(Duration::from_secs(600))),

            max_lifetime: parse_env_opt::<u64>("PG_MAX_LIFETIME_SECS")
                .map(Duration::from_secs)
                .or(Some(Duration::from_secs(1800))),

            statement_log_level: StatementLogLevel::Debug,

            slow_statement_threshold: Duration::from_millis(
                parse_env("PG_SLOW_STATEMENT_THRESHOLD_MS", 1000),
            ),
        }
    }
}

/// Pool tuning and connection settings shared across all shards in an
/// [`ApplicationSharded`] topology.
///
/// Per-shard connection URLs are provided separately via `shard_urls`. All
/// other settings are applied uniformly — shard pools are intended to be
/// symmetric replicas with identical capacity profiles.
///
/// # Environment variables
///
/// | Variable                        | Default |
/// |----------------------------------|---------|
/// | `PG_SHARD_COUNT`                | —       |
/// | `PG_SHARD_<N>_URL`             | —       |
/// | `PG_MAX_CONNECTIONS`            | 20      |
/// | `PG_MIN_CONNECTIONS`            | 2       |
/// | `PG_ACQUIRE_TIMEOUT_SECS`       | 5       |
/// | `PG_IDLE_TIMEOUT_SECS`          | 600     |
/// | `PG_MAX_LIFETIME_SECS`          | 1800    |
/// | `PG_SLOW_STATEMENT_THRESHOLD_MS`| 1000    |
///
/// [`ApplicationSharded`]: TopologyConfig::ApplicationSharded
#[derive(Debug, Clone)]
pub struct ShardedPostgresConfig {
    /// Number of application-level shards.  Must equal `shard_urls.len()`.
    pub shard_count: u16,

    /// Connection strings indexed by shard position: index `i` maps to
    /// [`ShardId(i)`].  Length must equal `shard_count`.
    ///
    /// [`ShardId(i)`]: crate::routing::ShardId
    pub shard_urls: Vec<String>,

    pub max_connections: u32,
    pub min_connections: u32,
    pub acquire_timeout: Duration,
    pub idle_timeout: Option<Duration>,
    pub max_lifetime: Option<Duration>,
    pub statement_log_level: StatementLogLevel,
    pub slow_statement_threshold: Duration,
}

impl ShardedPostgresConfig {
    /// Reads the sharded cluster configuration from environment variables.
    ///
    /// # Panics
    ///
    /// - If `PG_SHARD_COUNT` is absent or zero.
    /// - If any `PG_SHARD_<N>_URL` (for `N` in `0..shard_count`) is absent.
    pub fn from_env() -> Self {
        let shard_count: u16 = parse_env("PG_SHARD_COUNT", 0u16);
        assert!(
            shard_count > 0,
            "PG_SHARD_COUNT must be set and > 0 in ApplicationSharded topology"
        );

        let shard_urls: Vec<String> = (0..shard_count)
            .map(|i| {
                let var = format!("PG_SHARD_{i}_URL");
                std::env::var(&var)
                    .unwrap_or_else(|_| panic!("{var} must be set for shard {i}"))
            })
            .collect();

        Self {
            shard_count,
            shard_urls,
            max_connections: parse_env("PG_MAX_CONNECTIONS", 20),
            min_connections: parse_env("PG_MIN_CONNECTIONS", 2),
            acquire_timeout: Duration::from_secs(parse_env("PG_ACQUIRE_TIMEOUT_SECS", 5)),
            idle_timeout: parse_env_opt::<u64>("PG_IDLE_TIMEOUT_SECS")
                .map(Duration::from_secs)
                .or(Some(Duration::from_secs(600))),
            max_lifetime: parse_env_opt::<u64>("PG_MAX_LIFETIME_SECS")
                .map(Duration::from_secs)
                .or(Some(Duration::from_secs(1800))),
            statement_log_level: StatementLogLevel::Debug,
            slow_statement_threshold: Duration::from_millis(
                parse_env("PG_SLOW_STATEMENT_THRESHOLD_MS", 1000),
            ),
        }
    }
}

/// Selects the execution topology at service startup.
///
/// Resolved from the `PG_TOPOLOGY` environment variable:
///
/// | `PG_TOPOLOGY` value | Variant selected       |
/// |---------------------|------------------------|
/// | `"single"` or unset | `SingleNode`           |
/// | `"sharded"`         | `ApplicationSharded`   |
#[derive(Debug)]
pub enum TopologyConfig {
    /// A single global [`PgPool`] — the database engine handles routing.
    ///
    /// Ideal for CockroachDB, AWS Aurora, or any horizontally scalable
    /// distributed SQL engine where the application does not need to be
    /// aware of data placement.
    ///
    /// [`PgPool`]: sqlx::PgPool
    SingleNode(PostgresConfig),

    /// A registry of per-shard [`PgPool`]s — the application is responsible
    /// for routing each write to the correct shard pool.
    ///
    /// Use when running traditional sharded PostgreSQL (e.g., Citus,
    /// manual sharding) where the database does not self-route.
    ///
    /// [`PgPool`]: sqlx::PgPool
    ApplicationSharded(ShardedPostgresConfig),
}

impl TopologyConfig {
    /// Selects and fully populates the topology configuration from the process
    /// environment.
    ///
    /// Defaults to [`SingleNode`] when `PG_TOPOLOGY` is absent.
    ///
    /// [`SingleNode`]: TopologyConfig::SingleNode
    pub fn from_env() -> Self {
        match std::env::var("PG_TOPOLOGY").as_deref() {
            Ok("sharded") => {
                TopologyConfig::ApplicationSharded(ShardedPostgresConfig::from_env())
            }
            _ => TopologyConfig::SingleNode(PostgresConfig::from_env()),
        }
    }
}

// ── Internal helpers (unchanged) ─────────────────────────────────────────────

pub(crate) fn parse_env<T>(key: &str, default: T) -> T
where
    T: std::str::FromStr,
    T::Err: std::fmt::Debug,
{
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

pub(crate) fn parse_env_opt<T>(key: &str) -> Option<T>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Debug,
{
    std::env::var(key).ok().and_then(|v| v.parse().ok())
}
