// crates/shared-kernel/src/domain/repositories/cache.rs

use crate::errors::{AppError, AppResult, ErrorCode};
use async_trait::async_trait;
use serde::{Serialize, de::DeserializeOwned};
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

#[async_trait]
pub trait CacheRepositoryExt {
    async fn get_obj<T: DeserializeOwned>(&self, key: &str) -> AppResult<Option<T>>;
    async fn set_obj<T: Serialize + Send + Sync>(&self, key: &str, value: &T, ttl: Option<Duration>) -> AppResult<()>;
}

#[async_trait]
impl<R: CacheRepository + ?Sized> CacheRepositoryExt for R {
    async fn get_obj<T: DeserializeOwned>(&self, key: &str) -> AppResult<Option<T>> {
        match self.get(key).await? {
            Some(json) => {
                // On tente de désérialiser. Si ça échoue (donnée corrompue), 
                // on renvoie None plutôt que de faire crasher l'appli.
                Ok(serde_json::from_str(&json).ok())
            }
            None => Ok(None),
        }
    }

    async fn set_obj<T: Serialize + Send + Sync>(&self, key: &str, value: &T, ttl: Option<Duration>) -> AppResult<()> {
        let json = serde_json::to_string(value)
            .map_err(|e| AppError::new(ErrorCode::InternalError, format!("Serialization failed: {}", e)))?;
        
        self.set(key, &json, ttl).await
    }
}