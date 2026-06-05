// crates/geo_discovery/src/application/context/app.rs

use shared_kernel::idempotency::IdempotencyRepository;
use shared_kernel::types::{ProfileId, Region};
use std::sync::Arc;
use tokio::sync::mpsc::Sender;

use crate::application::context::{GeoDiscoveryCommandContext, GeoDiscoveryQueryContext};
use crate::handlers::HydrateTileCacheCommand;
use crate::repositories::{MapCacheRepository, MapPersistenceRepository};
use crate::resolvers::EngagementResolver;

#[derive(Clone)]
pub struct GeoDiscoveryAppContext {
    persistence_repo: Arc<dyn MapPersistenceRepository>,
    cache_repo: Arc<dyn MapCacheRepository>,
    idempotency_repo: Arc<dyn IdempotencyRepository>,
    engagement_resolver: Arc<dyn EngagementResolver>,
    hydration_sender: Sender<HydrateTileCacheCommand>,
}

impl GeoDiscoveryAppContext {
    pub fn new(
        persistence_repo: Arc<dyn MapPersistenceRepository>,
        cache_repo: Arc<dyn MapCacheRepository>,
        idempotency_repo: Arc<dyn IdempotencyRepository>,
        engagement_resolver: Arc<dyn EngagementResolver>,
        hydration_sender: Sender<HydrateTileCacheCommand>,
    ) -> Self {
        Self {
            persistence_repo,
            cache_repo,
            idempotency_repo,
            engagement_resolver,
            hydration_sender,
        }
    }

    pub fn query(&self, region: Region) -> GeoDiscoveryQueryContext {
        GeoDiscoveryQueryContext::new(self.clone(), region, self.hydration_sender.clone())
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

    pub fn engagement_resolver(&self) -> Arc<dyn EngagementResolver> {
        self.engagement_resolver.clone()
    }

    pub fn hydration_sender(&self) -> Sender<HydrateTileCacheCommand> {
        self.hydration_sender.clone()
    }
}
