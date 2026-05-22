// backend/services/profile/outbox_processor/src/main.rs

use std::env;
use std::time::Duration;
use tokio::sync::watch;

use infra_kafka::KafkaEventProducer;
use infra_sqlx::{PostgresOutboxRepository, sqlx::PgPool};
use shared_kernel::core::{Error, Result};
use shared_kernel::messaging::OutboxProcessor;

#[tokio::main]
async fn main() -> Result<()> {
    // 1. Initialisation du Tracing
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_target(false)
        .init();

    tracing::info!("📡 Starting Profile Outbox Producer Service...");

    // 2. Configuration (Récupérée explicitement)
    let db_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let brokers = env::var("KAFKA_BROKERS").unwrap_or_else(|_| "localhost:9092".into());

    let batch_size: usize = env::var("OUTBOX_BATCH_SIZE")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(100);

    let interval_ms: u64 = env::var("OUTBOX_POLLING_MS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(500);

    // 3. Montage de l'infrastructure
    let pool = PgPool::connect(&db_url).await.map_err(|e| {
        tracing::error!("Failed to connect to Postgres: {}", e);
        Error::internal(e.to_string())
    })?;

    let store = PostgresOutboxRepository::new(pool);
    let producer = KafkaEventProducer::new(&brokers, "profile.events".to_string()).await?;

    // 4. Configuration du processeur
    let processor = OutboxProcessor::new(
        store,
        producer,
        batch_size as u32,
        Duration::from_millis(interval_ms),
    );

    // 5. Gestion du Graceful Shutdown
    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    // Tâche de surveillance du signal système
    tokio::spawn(async move {
        match tokio::signal::ctrl_c().await {
            Ok(()) => {
                tracing::warn!("🛑 Shutdown signal received, stopping producer ...");
                let _ = shutdown_tx.send(true);
            }
            Err(e) => tracing::error!("❌ Unable to listen for shutdown signal: {}", e),
        }
    });

    tracing::info!(
        "✅ Processor active: batch_size={}, interval={}ms",
        batch_size,
        interval_ms
    );

    // 6. Exécution
    processor.run(shutdown_rx).await;

    tracing::info!("👋 Profile Outbox Producer exited cleanly.");
    Ok(())
}
