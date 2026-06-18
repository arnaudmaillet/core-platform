// crates/post/core/src/post/infrastructure/messaging/eviction_handler.rs

use crate::post::decorators::cached_read_repository::CachedPostReadRepository;
use shared_kernel::cache::repository::CacheRepository;
use shared_kernel::core::{Error, Result};
use shared_kernel::messaging::EventEnvelope;
use shared_kernel::types::PostId;
use std::sync::Arc;

pub struct PostCacheEvictionHandler {
    cache: Arc<dyn CacheRepository>,
}

impl PostCacheEvictionHandler {
    pub fn new(cache: Arc<dyn CacheRepository>) -> Self {
        Self { cache }
    }

    pub async fn handle(&self, envelope: EventEnvelope) -> Result<()> {
        let post_id_str = envelope
            .payload
            .get("post_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                Error::validation(
                    "post_id",
                    "Identifiant 'post_id' manquant dans l'événement CDC",
                )
            })?;

        let post_id = PostId::try_new(post_id_str)?;
        let cache_key = format!("posts:atom:{}", post_id);
        self.cache.delete(&cache_key).await?;

        tracing::debug!(%post_id, %cache_key, "Post cache successfully evicted asynchronously by CDC handler");
        Ok(())
    }
}
