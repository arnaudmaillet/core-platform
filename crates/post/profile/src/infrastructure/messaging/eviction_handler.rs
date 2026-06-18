// crates/post/profile/src/infrastructure/messaging/eviction_handler.rs

use crate::infrastructure::redis::RedisProfileEvictor;
use shared_kernel::core::{Error, Result};
use shared_kernel::messaging::EventEnvelope;
use shared_kernel::types::ProfileId;

pub struct ProfileCacheEvictionHandler {
    redis_evictor: RedisProfileEvictor,
}

impl ProfileCacheEvictionHandler {
    pub fn new(redis_evictor: RedisProfileEvictor) -> Self {
        Self { redis_evictor }
    }

    pub async fn handle(&self, envelope: EventEnvelope) -> Result<()> {
        // Dans un flux CDC, on extrait généralement l'ID de la ligne modifiée
        let profile_id_str = envelope
            .payload
            .get("profile_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::validation("profile_id", "Missing profile_id in CDC event"))?;

        let profile_id = ProfileId::try_new(profile_id_str)?;
        self.redis_evictor.evict(&profile_id).await?;

        Ok(())
    }
}
