// crates/geo_discovery/src/application/context/app.rs

use std::sync::Arc;
use shared_kernel::idempotency::IdempotencyRepository;
use shared_kernel::types::{ProfileId, Region};

use crate::repositories::{MapCacheRepository, MapPersistenceRepository};
use crate::application::context::{GeoDiscoveryCommandContext, GeoDiscoveryQueryContext};

#[derive(Clone)]
pub struct GeoDiscoveryAppContext {
    persistence_repo: Arc<dyn MapPersistenceRepository>,
    cache_repo: Arc<dyn MapCacheRepository>,
    idempotency_repo: Arc<dyn IdempotencyRepository>,
}

impl GeoDiscoveryAppContext {
    pub fn new(
        persistence_repo: Arc<dyn MapPersistenceRepository>,
        cache_repo: Arc<dyn MapCacheRepository>,
        idempotency_repo: Arc<dyn IdempotencyRepository>,
    ) -> Self {
        Self {
            persistence_repo,
            cache_repo,
            idempotency_repo,
        }
    }

    pub fn query(&self, region: Region) -> GeoDiscoveryQueryContext {
        GeoDiscoveryQueryContext::new(self.clone(), region)
    }

    pub fn command(&self, operator_id: ProfileId, region: Region) -> GeoDiscoveryCommandContext {
        GeoDiscoveryCommandContext::new(self.clone(), operator_id, region)
    }

    pub fn persistence_repo(&self) -> Arc<dyn MapPersistenceRepository> {
        self.persistence_repo.clone()
    }

    pub fn cache_repo(&self) -> Arc<dyn MapCacheRepository> {
        self.cache_repo.clone()
    }

    pub fn idempotency_repo(&self) -> Arc<dyn IdempotencyRepository> {
        self.idempotency_repo.clone()
    }
}