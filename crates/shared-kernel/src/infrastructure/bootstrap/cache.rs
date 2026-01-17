// crates/shared-kernel/src/infrastructure/bootstrap/cache.rs

#![cfg(all(feature = "kafka", feature = "redis"))]

use std::sync::Arc;
use std::env;
use crate::errors::AppResult;
use crate::infrastructure::kafka::KafkaMessageConsumer;
use crate::application::workers::CacheWorker;
use crate::infrastructure::redis::repositories::RedisCacheRepository;

pub async fn run_cache_worker(
    service_name: &str,
    topic: &str,
    group_id: &str,
) -> AppResult<()> {
    tracing_subscriber::fmt::init();
    log::info!("🚀 Starting {} Cache Worker...", service_name);

    let redis_url = env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".into());
    let brokers = env::var("KAFKA_BROKERS").unwrap_or_else(|_| "localhost:9092".into());

    // Limite de parallélisme pour Redis
    let max_concurrency = env::var("CACHE_MAX_CONCURRENCY")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(500);

    let redis_instance = RedisCacheRepository::new(&redis_url).await?;

    let redis_repo: Arc<dyn crate::domain::repositories::CacheRepository> = Arc::new(redis_instance);
    let kafka_consumer = Arc::new(KafkaMessageConsumer::new(&brokers, group_id, max_concurrency));

    let worker = CacheWorker::new(kafka_consumer.clone(), redis_repo);

    // Gestion du Shutdown
    let kafka_for_shutdown = kafka_consumer.clone();
    tokio::spawn(async move {
        if let Ok(_) = tokio::signal::ctrl_c().await {
            log::warn!("🛑 Shutdown signal received, stopping Kafka consumer...");
            kafka_for_shutdown.stop();
        }
    });

    log::info!("✅ Cache worker active (concurrency: {})", max_concurrency);
    worker.start(topic).await
}