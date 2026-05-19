// crates/shared-kernel/src/application/workers/cache_worker.rs

use crate::cache::CacheRepository;
use crate::core::Result;
use crate::messaging::{EventConsumer, EventHandler};
use std::sync::Arc;

pub struct CacheWorker {
    consumer: Arc<dyn EventConsumer>,
    cache_repo: Arc<dyn CacheRepository>,
}

impl CacheWorker {
    pub fn new(consumer: Arc<dyn EventConsumer>, cache_repo: Arc<dyn CacheRepository>) -> Self {
        Self {
            consumer,
            cache_repo,
        }
    }

    pub async fn start(&self, topic: &str) -> Result<()> {
        log::info!("🚀 CacheWorker starting for topic: {}", topic);

        let repo = Arc::clone(&self.cache_repo);

        let handler: EventHandler = Box::new(move |envelope| {
            let cache = Arc::clone(&repo);

            Box::pin(async move {
                let pattern = format!("{}:{}*", envelope.aggregate_type, envelope.aggregate_id);
                cache.invalidate_pattern(&pattern).await?;

                Ok(())
            })
        });

        self.consumer.consume(topic, handler).await
    }
}
