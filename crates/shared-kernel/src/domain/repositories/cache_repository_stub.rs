// crates/shared-kernel/src/utils/cache_repository_stub.rs

use crate::core::{Error, Result};
use crate::domain::repositories::CacheRepository;
use async_trait::async_trait;
use std::sync::Mutex;
use std::time::Duration;

#[derive(Default)]
pub struct CacheRepositoryStub {
    pub storage: Mutex<std::collections::HashMap<String, String>>,
    pub fail_all: bool,
}

impl CacheRepositoryStub {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl CacheRepository for CacheRepositoryStub {
    // Note : Pense à mettre à jour le trait CacheRepository avec Result<()> également
    async fn set(&self, key: &str, value: &str, _ttl: Option<Duration>) -> Result<()> {
        if self.fail_all {
            return Err(Error::internal("Cache Down"));
        }
        self.storage
            .lock()
            .unwrap()
            .insert(key.to_string(), value.to_string());
        Ok(())
    }

    async fn get(&self, key: &str) -> Result<Option<String>> {
        if self.fail_all {
            return Err(Error::internal("Cache Down"));
        }
        Ok(self.storage.lock().unwrap().get(key).cloned())
    }

    async fn delete(&self, key: &str) -> Result<()> {
        if self.fail_all {
            return Err(Error::internal("Cache Down"));
        }
        self.storage.lock().unwrap().remove(key);
        Ok(())
    }

    async fn exists(&self, key: &str) -> Result<bool> {
        if self.fail_all {
            return Err(Error::internal("Cache Down"));
        }
        Ok(self.storage.lock().unwrap().contains_key(key))
    }

    async fn set_many(&self, entries: Vec<(&str, String)>, _ttl: Option<Duration>) -> Result<()> {
        if self.fail_all {
            return Err(Error::internal("Cache Down"));
        }

        let mut map = self.storage.lock().unwrap();
        for (key, value) in entries {
            map.insert(key.to_string(), value);
        }

        Ok(())
    }

    async fn invalidate_pattern(&self, pattern: &str) -> Result<()> {
        if self.fail_all {
            return Err(Error::internal("Cache Down"));
        }

        let mut map = self.storage.lock().unwrap();
        let prefix = pattern.replace("*", "");
        map.retain(|key, _| !key.starts_with(&prefix));

        Ok(())
    }
}
