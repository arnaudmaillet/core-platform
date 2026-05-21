// crates/shared-kernel/src/application/cache/repository.rs

use crate::core::{Error, Result}; // Importation du nouveau système unifié
use async_trait::async_trait;
use serde::{Serialize, de::DeserializeOwned};
use std::time::Duration;

#[async_trait]
pub trait CacheRepository: Send + Sync {
    async fn set(&self, key: &str, value: &str, ttl: Option<Duration>) -> Result<()>;
    async fn get(&self, key: &str) -> Result<Option<String>>;
    async fn delete(&self, key: &str) -> Result<()>;
    async fn exists(&self, key: &str) -> Result<bool>;
    async fn set_many(&self, entries: Vec<(&str, String)>, ttl: Option<Duration>) -> Result<()>;
    async fn invalidate_pattern(&self, pattern: &str) -> Result<()>;
}

#[async_trait]
pub trait CacheRepositoryExt {
    async fn get_obj<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>>;
    async fn set_obj<T: Serialize + Send + Sync>(
        &self,
        key: &str,
        value: &T,
        ttl: Option<Duration>,
    ) -> Result<()>;
}

#[async_trait]
impl<R: CacheRepository + ?Sized> CacheRepositoryExt for R {
    async fn get_obj<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>> {
        match self.get(key).await? {
            Some(json) => Ok(serde_json::from_str(&json).ok()),
            None => Ok(None),
        }
    }

    async fn set_obj<T: Serialize + Send + Sync>(
        &self,
        key: &str,
        value: &T,
        ttl: Option<Duration>,
    ) -> Result<()> {
        let json = serde_json::to_string(value)
            .map_err(|e| Error::internal(format!("Cache serialization failed: {}", e)))?;

        self.set(key, &json, ttl).await
    }
}
