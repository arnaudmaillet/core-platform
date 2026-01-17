// crates/shared-kernel/src/application/workers/cache_worker.rs

use std::sync::Arc;
use crate::application::ports::{MessageConsumer, MessageHandler};
use crate::domain::repositories::CacheRepository;
use crate::errors::AppResult;

pub struct CacheWorker {
    consumer: Arc<dyn MessageConsumer>,
    cache_repo: Arc<dyn CacheRepository>,
}

impl CacheWorker {
    pub fn new(
        consumer: Arc<dyn MessageConsumer>,
        cache_repo: Arc<dyn CacheRepository>
    ) -> Self {
        Self { consumer, cache_repo }
    }

    pub async fn start(&self, topic: &str) -> AppResult<()> {
        log::info!("🚀 CacheWorker starting for topic: {}", topic);

        let repo = Arc::clone(&self.cache_repo);

        let handler: MessageHandler = Box::new(move |envelope| {
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