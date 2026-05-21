// crates/shared-kernel/src/application/cache/worker.rs

use crate::cache::CacheRepository;
use crate::core::Result;
use crate::messaging::{EventConsumer, EventHandler};
use std::sync::Arc;
use tracing::{error, info, instrument};

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
        info!(topic = %topic, "🚀 CacheWorker starting consumption");

        let repo = Arc::clone(&self.cache_repo);

        let handler: EventHandler = Box::new(move |envelope| {
            let cache = Arc::clone(&repo);
            let aggregate_type = envelope.aggregate_type.clone();
            let aggregate_id = envelope.aggregate_id.clone();

            Box::pin(async move {
                Self::process_invalidation(cache, aggregate_type, aggregate_id).await
            })
        });

        self.consumer.consume(topic, handler).await
    }

    // L'attribut instrument crée un "span" de tracing automatique
    // Cela permet de retrouver tous les logs de cette fonction avec aggregate_id dans les outils de log (ex: Jaeger/Datadog)
    #[instrument(skip(cache), fields(aggregate_type = %agg_type, aggregate_id = %agg_id))]
    async fn process_invalidation(
        cache: Arc<dyn CacheRepository>,
        agg_type: String,
        agg_id: String,
    ) -> Result<()> {
        let pattern = format!("{}:{}*", agg_type, agg_id);

        info!(pattern = %pattern, "🧹 Invalidating cache pattern");

        if let Err(e) = cache.invalidate_pattern(&pattern).await {
            error!(error = %e, "❌ Failed to invalidate cache pattern");
            return Err(e);
        }

        info!("✅ Cache pattern successfully invalidated");
        Ok(())
    }
}
