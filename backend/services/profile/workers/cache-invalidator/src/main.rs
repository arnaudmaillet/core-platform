// backend/services/profile/cache_invalidator/src/main.rs

use std::sync::Arc;

use infra_fred::RedisCacheRepository;
use infra_kafka::KafkaEventConsumer;
use shared_kernel::cache::CacheWorker;
use shared_kernel::core::{Error, Result};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_target(false)
        .init();

    let redis_url = std::env::var("REDIS_URL").expect("REDIS_URL must be set");
    let brokers = std::env::var("KAFKA_BROKERS").expect("KAFKA_BROKERS must be set");

    let redis = RedisCacheRepository::new(&redis_url).await?;
    let consumer = KafkaEventConsumer::new(&brokers, "profile-cache-group", 500);

    let worker = CacheWorker::new(Arc::new(consumer), Arc::new(redis));
    let worker_handle = tokio::spawn(async move { worker.start("profile.events").await });

    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            tracing::info!("🛑 Shutdown signal received, exiting...");
            Ok(())
        }
        res = worker_handle => {
            match res {
                Ok(worker_result) => worker_result,
                Err(join_err) => {
                    tracing::error!("Worker task panicked or failed to join: {:?}", join_err);
                    Err(Error::internal("Worker task failed"))
                }
            }
        }
    }
}
