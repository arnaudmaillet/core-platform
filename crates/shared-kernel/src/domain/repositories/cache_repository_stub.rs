// crates/shared-kernel/src/utils/cache_repository_stub.rs

use crate::domain::repositories::CacheRepository;
use crate::errors::{AppError, AppResult, ErrorCode};
use async_trait::async_trait;
use std::sync::Mutex;
use std::time::Duration;

pub struct CacheRepositoryStub {
    pub storage: Mutex<std::collections::HashMap<String, String>>,
    pub fail_all: bool,
}

impl Default for CacheRepositoryStub {
    fn default() -> Self {
        Self {
            storage: Mutex::new(std::collections::HashMap::new()),
            fail_all: false,
        }
    }
}

#[async_trait]
impl CacheRepository for CacheRepositoryStub {
    async fn set(&self, key: &str, value: &str, _ttl: Option<Duration>) -> AppResult<()> {
        if self.fail_all {
            return Err(AppError::new(ErrorCode::InternalError, "Cache Down"));
        }
        self.storage
            .lock()
            .unwrap()
            .insert(key.to_string(), value.to_string());
        Ok(())
    }

    async fn get(&self, key: &str) -> AppResult<Option<String>> {
        if self.fail_all {
            return Err(AppError::new(ErrorCode::InternalError, "Cache Down"));
        }
        Ok(self.storage.lock().unwrap().get(key).cloned())
    }

    async fn delete(&self, key: &str) -> AppResult<()> {
        if self.fail_all {
            return Err(AppError::new(ErrorCode::InternalError, "Cache Down"));
        }
        self.storage.lock().unwrap().remove(key);
        Ok(())
    }

    async fn invalidate_pattern(&self, pattern: &str) -> AppResult<()> {
        if self.fail_all {
            return Err(AppError::new(ErrorCode::InternalError, "Cache Down"));
        }

        let mut map = self.storage.lock().unwrap();
        // Simulation simple du pattern Redis '*' par un prefix match
        let prefix = pattern.replace("*", "");
        map.retain(|key, _| !key.starts_with(&prefix));

        Ok(())
    }
}
