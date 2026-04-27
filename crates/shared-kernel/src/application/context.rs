// crates/shared-kernel/src/application/context.rs

use crate::domain::repositories::CacheRepository;
use std::sync::Arc;

#[derive(Clone)]
pub struct BaseAppContext {
    pool: Option<sqlx::PgPool>,
    cache: Arc<dyn CacheRepository>,
}

impl BaseAppContext {
    pub fn new(
        pool: Option<sqlx::PgPool>,
        cache: Arc<dyn CacheRepository>,
    ) -> Self {
        Self {
            pool,
            cache,
        }
    }

    pub fn pool(&self) -> Option<&sqlx::PgPool> {
        self.pool.as_ref()
    }

    pub fn cache(&self) -> Arc<dyn CacheRepository> {
        self.cache.clone()
    }
}
