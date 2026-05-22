// crates/shared-kernel/src/infrastructure/redis/factories/redis_context.rs

use crate::{RedisCacheRepository, RedisConfig, RedisContextBuilder};
use shared_kernel::core::{Error, ErrorCode, Result};
use std::sync::Arc;

pub struct RedisContext {
    repository: Arc<RedisCacheRepository>,
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

    pub fn repository(&self) -> Arc<RedisCacheRepository> {
        self.repository.clone()
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
                format!("Failed to connect to Redis at {}: {}", builder.url, e),
            )
        })?;

        Ok(Self {
            repository: Arc::new(repository),
            url: builder.url,
            max_clients: builder.max_clients,
        })
    }
}
