// crates/shared-kernel/src/building_blocks/messaging/outbox/relay.rs

#![cfg(all(feature = "postgres", feature = "kafka"))]

use crate::core::{Error, Result};
use crate::kafka::KafkaEventProducer;
use crate::messaging::OutboxProcessor;
use crate::postgres::PostgresOutboxRepository;
use sqlx::PgPool;
use std::env;
use std::time::Duration;

pub async fn run_outbox_relay(domain_name: &str, default_topic: &str) -> Result<()> {
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
    let pool = PgPool::connect(&db_url)
        .await
        .map_err(|e| Error::internal(e.to_string()))?;

    let store = PostgresOutboxRepository::new(pool);
    let producer = KafkaEventProducer::new(&brokers, default_topic.to_string()).await?;

    // 4. Configuration du processeur avec les paramètres extraits
    let processor = OutboxProcessor::new(
        store,
        producer,
        batch_size,
        Duration::from_millis(interval_ms),
    );

    // 5. Préparation du signal d'arrêt (Graceful Shutdown)
    // On crée un canal "watch" pour notifier le processeur
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

    // 6. Gestionnaire de signaux système (Ctrl+C, SIGTERM)
    // On lance une tâche qui attend un signal et change la valeur du watch
    tokio::spawn(async move {
        match tokio::signal::ctrl_c().await {
            Ok(()) => {
                tracing::info!("🛑 Shutdown signal received, stopping relay...");
                let _ = shutdown_tx.send(true);
            }
            Err(err) => {
                tracing::error!("❌ Unable to listen for shutdown signal: {}", err);
            }
        }
    });

    tracing::info!(
        "✅ Processor configured: batch_size={}, interval={}ms",
        batch_size,
        interval_ms
    );

    // 7. Exécution
    // On passe le shutdown_rx au processeur
    processor.run(shutdown_rx).await;

    tracing::info!("👋 Outbox relay for {} exited clean", domain_name);
    Ok(())
}
