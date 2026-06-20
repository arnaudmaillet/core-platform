use postgres::{PostgresConfig, pool::PgPoolBuilder};
use sqlx::PgPool;
use std::sync::OnceLock;
use tracing_subscriber::EnvFilter;

static TRACING: OnceLock<()> = OnceLock::new();

/// Installs a minimal `tracing` subscriber for tests.
///
/// Idempotent: the first caller wins; subsequent calls are no-ops.
/// Reads `RUST_LOG` to adjust the filter level (defaults to `debug`).
pub fn init_tracing() {
    TRACING.get_or_init(|| {
        let filter = EnvFilter::try_from_env("RUST_LOG")
            .unwrap_or_else(|_| EnvFilter::new("debug"));
        let _ = tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_test_writer()
            .try_init();
    });
}

/// Builds a small connection pool backed by `DATABASE_URL`.
///
/// # Panics
///
/// Panics if `DATABASE_URL` is not set or the database is unreachable.
/// Integration tests are expected to run against a real PostgreSQL instance.
pub async fn test_pool() -> PgPool {
    init_tracing();

    let config = PostgresConfig {
        database_url: std::env::var("DATABASE_URL")
            .expect("DATABASE_URL must be set to run integration tests"),
        max_connections: 5,
        min_connections: 1,
        acquire_timeout: std::time::Duration::from_secs(5),
        idle_timeout: None,
        max_lifetime: None,
        statement_log_level: postgres::config::StatementLogLevel::Debug,
        slow_statement_threshold: std::time::Duration::from_millis(500),
    };

    PgPoolBuilder::build(config)
        .await
        .expect("failed to connect to the test database")
}
