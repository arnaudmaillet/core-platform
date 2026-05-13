// crates/shared-kernel/src/infrastructure/bootstrap/cache.rs

#![cfg(all(feature = "kafka", feature = "redis"))]

use crate::cache::{CacheRepository, CacheWorker};
use crate::core::Result;
use crate::kafka::KafkaEventConsumer;
use crate::redis::RedisCacheRepository;
use std::env;
use std::sync::Arc;

pub async fn run_cache_worker(service_name: &str, topic: &str, group_id: &str) -> Result<()> {
    tracing_subscriber::fmt::init();
    log::info!("🚀 Starting {} Cache Worker...", service_name);

    let redis_url =
        env::var("PROFILE_REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".into());
    let brokers = env::var("KAFKA_BROKERS").unwrap_or_else(|_| "localhost:9092".into());

    // Limite de parallélisme pour Redis
    let max_concurrency = env::var("CACHE_MAX_CONCURRENCY")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(500);

    let redis_instance = RedisCacheRepository::new(&redis_url).await?;

    let redis_repo: Arc<dyn CacheRepository> = Arc::new(redis_instance);
    let kafka_consumer = Arc::new(KafkaEventConsumer::new(&brokers, group_id, max_concurrency));

    let worker = CacheWorker::new(kafka_consumer.clone(), redis_repo);

    // Gestion du Shutdown
    let kafka_for_shutdown = kafka_consumer.clone();
    tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            log::warn!("🛑 Shutdown signal received, stopping Kafka consumer...");
            kafka_for_shutdown.stop();
        }
    });

    log::info!("✅ Cache worker active (concurrency: {})", max_concurrency);
    worker.start(topic).await
}
