// backend/services/account/workers/janitor/main.rs

use account_old::db::PostgresGlobalIdentityRegistry;
use account_old::workers::GlobalRegistryJanitor;
use infra_sqlx::sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    tracing::info!("Starting Global Registry Janitor standalone binary...");

    let global_db_url = std::env::var("GLOBAL_DATABASE_URL")
        .expect("GLOBAL_DATABASE_URL environment variable must be set");

    let global_pool = PgPoolOptions::new()
        .max_connections(2)
        .acquire_timeout(Duration::from_secs(5))
        .connect(&global_db_url)
        .await?;

    tracing::debug!("Connected to the global database registry successfully");

    let global_registry = Arc::new(PostgresGlobalIdentityRegistry::new(global_pool));

    let purge_interval_secs = std::env::var("JANITOR_PURGE_INTERVAL_SECS")
        .unwrap_or_else(|_| "60".to_string())
        .parse::<u64>()?;

    let retention_mins = std::env::var("JANITOR_RETENTION_MINS")
        .unwrap_or_else(|_| "15".to_string())
        .parse::<i64>()?;

    let janitor = GlobalRegistryJanitor::new(
        global_registry,
        Duration::from_secs(purge_interval_secs),
        chrono::Duration::minutes(retention_mins),
    );

    janitor.run().await;

    Ok(())
}
