// crates/shared-kernel/src/infrastructure/redis/factories/redis_context_builder.rs

use crate::errors::{AppError, AppResult, ErrorCode};
use crate::infrastructure::redis::factories::RedisContext;

pub struct RedisContextBuilder {
    pub(crate) url: String,
    pub(crate) max_clients: usize,
}

impl Default for RedisContextBuilder {
    fn default() -> Self {
        Self {
            url: "redis://127.0.0.1:6379".to_string(),
            max_clients: 16,
        }
    }
}

impl RedisContextBuilder {
    pub fn new() -> AppResult<Self> {
        let mut builder = Self::default();

        builder.url = std::env::var("PROFILE_REDIS_URL")
            .map_err(|_| AppError::new(ErrorCode::InternalError, "PROFILE_REDIS_URL must be set"))?;

        if let Ok(max) = std::env::var("PROFILE_REDIS_MAX_CLIENTS") {
            if let Ok(val) = max.parse::<usize>() {
                builder.max_clients = val;
            }
        }

        Ok(builder)
    }

    pub fn with_url(mut self, url: impl Into<String>) -> Self {
        self.url = url.into();
        self
    }

    pub fn with_max_clients(mut self, max: usize) -> Self {
        self.max_clients = max;
        self
    }

    pub async fn build(self) -> AppResult<RedisContext> {
        RedisContext::restore(self).await
    }
}