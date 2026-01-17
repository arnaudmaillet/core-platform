// crates/shared-kernel/src/infrastructure/bootstrap/outbox.rs

#![cfg(all(feature = "postgres", feature = "kafka"))]

use std::time::Duration;
use std::env;
use sqlx::PgPool;
use crate::errors::AppResult;
use crate::infrastructure::kafka::KafkaMessageProducer;
use crate::infrastructure::OutboxProcessor;
use crate::infrastructure::postgres::PostgresOutboxStore;

pub async fn run_outbox_relay(
    domain_name: &str,
    default_topic: &str,
) -> AppResult<()> {
    // 1. Initialisation des logs
    tracing_subscriber::fmt::init();
    tracing::info!("📡 Starting Outbox Relay for domain: {}", domain_name);

    // 2. Configuration via Environnement (avec valeurs par défaut)
    let db_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let brokers = env::var("KAFKA_BROKERS").unwrap_or_else(|_| "localhost:9092".to_string());

    // Tuning des performances
    let batch_size = env::var("OUTBOX_BATCH_SIZE")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(100);

    let interval_ms = env::var("OUTBOX_POLLING_MS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(500);

    // 3. Montage de l'infrastructure
    let pool = PgPool::connect(&db_url).await
        .map_err(|e| crate::errors::AppError::new(crate::errors::ErrorCode::InternalError, e.to_string()))?;

    let store = PostgresOutboxStore::new(pool);
    let producer = KafkaMessageProducer::new(&brokers, default_topic.to_string()).await?;

    // 4. Configuration du processeur avec les paramètres extraits
    let processor = OutboxProcessor::new(
        store,
        producer,
        batch_size,
        Duration::from_millis(interval_ms)
    );

    tracing::info!(
        "✅ Processor configured: batch_size={}, interval={}ms",
        batch_size,
        interval_ms
    );

    // 5. Exécution
    processor.run().await;
    Ok(())
}