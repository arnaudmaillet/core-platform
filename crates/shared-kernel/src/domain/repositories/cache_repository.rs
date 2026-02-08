// crates/shared-kernel/src/domain/repositories/cache.rs

use crate::errors::AppResult;
use async_trait::async_trait;
use std::time::Duration;

#[async_trait]
pub trait CacheRepository: Send + Sync {
    async fn set(&self, key: &str, value: &str, ttl: Option<Duration>) -> AppResult<()>;
    async fn get(&self, key: &str) -> AppResult<Option<String>>;
    async fn delete(&self, key: &str) -> AppResult<()>;
    async fn exists(&self, key: &str) -> AppResult<bool>;
    async fn set_many(&self, entries: Vec<(&str, String)>, ttl: Option<Duration>) -> AppResult<()>;
    async fn invalidate_pattern(&self, pattern: &str) -> AppResult<()>;
}