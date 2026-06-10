// crates/shared-kernel/src/infrastructure/redis/factories/redis_context.rs

use crate::{RedisCacheRepository, RedisConfig, RedisContextBuilder, RedisIdempotencyRepository};
use shared_kernel::core::{Error, ErrorCode, Result};
use std::sync::Arc;

pub struct RedisContext {
    cache_repository: Arc<RedisCacheRepository>,
    idempotency_repository: Arc<RedisIdempotencyRepository>,
    url: String,
    max_clients: usize,
}

impl RedisContext {
    pub fn builder() -> Result<RedisContextBuilder> {
        RedisContextBuilder::new()
    }

    pub fn builder_raw() -> RedisContextBuilder {
        RedisContextBuilder::default()
    }

    pub fn cache_repository(&self) -> Arc<RedisCacheRepository> {
        self.cache_repository.clone()
    }

    pub fn idempotency_repository(&self) -> Arc<RedisIdempotencyRepository> {
        self.idempotency_repository.clone()
    }

    pub fn url(&self) -> String {
        self.url.clone()
    }

    pub fn config(&self) -> RedisConfig {
        RedisConfig {
            max_clients: self.max_clients,
        }
    }

    pub(crate) async fn restore(builder: RedisContextBuilder) -> Result<Self> {
        let repository = RedisCacheRepository::new(&builder.url).await.map_err(|e| {
            Error::new(
                ErrorCode::InternalError,
                format!("Failed to connect to Redis Cache at {}: {}", builder.url, e),
            )
        })?;

        let raw_pool = repository.pool().clone();

        let idempotency =
            RedisIdempotencyRepository::new(raw_pool, "core-platform-idempotency", 86400);

        Ok(Self {
            cache_repository: Arc::new(repository),
            idempotency_repository: Arc::new(idempotency),
            url: builder.url,
            max_clients: builder.max_clients,
        })
    }
}
