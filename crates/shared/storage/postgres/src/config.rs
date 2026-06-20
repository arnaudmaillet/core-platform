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

/// Full configuration surface for the PostgreSQL connection pool.
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

fn parse_env<T>(key: &str, default: T) -> T
where
    T: std::str::FromStr,
    T::Err: std::fmt::Debug,
{
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn parse_env_opt<T>(key: &str) -> Option<T>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Debug,
{
    std::env::var(key).ok().and_then(|v| v.parse().ok())
}
