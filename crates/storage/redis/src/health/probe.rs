//! Ready-made [`HealthProbe`] over a Redis client, so services don't hand-roll
//! the readiness closure. Accepts a [`RedisClient`] by value (cheap `Arc`-backed
//! clone); callers holding an `Arc<RedisClient>` pass `(*client).clone()`.

use std::sync::Arc;

use async_trait::async_trait;
use health::HealthProbe;

use crate::RedisClient;

struct RedisHealthProbe {
    client: RedisClient,
}

#[async_trait]
impl HealthProbe for RedisHealthProbe {
    fn name(&self) -> &str {
        "redis"
    }

    async fn check(&self) -> anyhow::Result<()> {
        super::check::health_check(&*self.client)
            .await
            .map_err(|e| anyhow::anyhow!("redis: {e}"))
    }
}

/// Builds a readiness probe (name `"redis"`) over a live Redis client.
pub fn probe(client: RedisClient) -> Arc<dyn HealthProbe> {
    Arc::new(RedisHealthProbe { client })
}
