// crates/shared-kernel/src/infrastructure/redis/factories/mod.rs

use crate::errors::{AppError, AppResult, ErrorCode};
use crate::infrastructure::redis::repositories::RedisCacheRepository;
use std::sync::Arc;

pub struct RedisConfig {
    pub url: String,
    pub max_clients: usize,
}

impl RedisConfig {
    pub fn from_env() -> AppResult<Self> {
        Ok(Self {
            url: std::env::var("REDIS_URL")
                .map_err(|_| AppError::new(ErrorCode::InternalError, "REDIS_URL must be set"))?,
            max_clients: std::env::var("REDIS_MAX_CLIENTS")
                .unwrap_or_else(|_| "16".to_string())
                .parse()
                .map_err(|_| AppError::new(ErrorCode::InternalError, "Invalid REDIS_MAX_CLIENTS"))?,
        })
    }
}

pub async fn create_redis_repository(config: &RedisConfig) -> AppResult<Arc<RedisCacheRepository>> {
    let repo = RedisCacheRepository::new(&config.url).await
        .map_err(|e| AppError::new(
            ErrorCode::InternalError,
            format!("Failed to connect to Redis: {}", e)
        ))?;

    Ok(Arc::new(repo))
}