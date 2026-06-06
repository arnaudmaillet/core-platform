// crates/geo_discovery/src/application/builder.rs

use infra_fred::fred::clients::Pool;
use infra_scylla::scylla::client::session::Session as ScyllaSession;
use shared_kernel::idempotency::IdempotencyRepository;
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::context::GeoDiscoveryAppContext;
use crate::handlers::{HydrateTileCacheCommand, HydrateTileCacheHandler};
use crate::infrastructure::repositories::{FredMapCacheRepository, ScyllaMapPersistenceRepository};
use crate::repositories::{MapCacheRepository, MapPersistenceRepository};
use crate::resolvers::EngagementResolver;
use crate::workers::MapCacheHydrationWorker;

pub struct GeoDiscoveryServiceBuilder {
    scylla_session: Arc<ScyllaSession>,
    fred_pool: Pool,
    idempotency_repo: Arc<dyn IdempotencyRepository>,
    engagement_resolver: Arc<dyn EngagementResolver>,
    max_posts_per_tile: usize,
}

impl GeoDiscoveryServiceBuilder {
    pub fn new(
        scylla_session: Arc<ScyllaSession>,
        fred_pool: Pool,
        idempotency_repo: Arc<dyn IdempotencyRepository>,
        engagement_resolver: Arc<dyn EngagementResolver>,
        max_posts_per_tile: usize,
    ) -> Self {
        Self {
            scylla_session,
            fred_pool,
            idempotency_repo,
            engagement_resolver,
            max_posts_per_tile,
        }
    }

    pub async fn build_context(
        &self,
    ) -> Result<Arc<GeoDiscoveryAppContext>, infra_scylla::scylla::errors::PrepareError> {
        let persistence_repo: Arc<dyn MapPersistenceRepository> =
            Arc::new(ScyllaMapPersistenceRepository::new(self.scylla_session.clone()).await?);

        let cache_repo: Arc<dyn MapCacheRepository> =
            Arc::new(FredMapCacheRepository::new(self.fred_pool.clone()));

        let (hydration_sender, hydration_receiver) =
            mpsc::channel::<HydrateTileCacheCommand>(10000);

        let hydration_handler = HydrateTileCacheHandler::new(
            cache_repo.clone(),
            persistence_repo.clone(),
            self.max_posts_per_tile,
        );

        let hydration_worker = MapCacheHydrationWorker::new(hydration_handler);
        hydration_worker.start(hydration_receiver);

        Ok(Arc::new(GeoDiscoveryAppContext::new(
            persistence_repo,
            cache_repo,
            self.idempotency_repo.clone(),
            self.engagement_resolver.clone(),
            hydration_sender,
        )))
    }
}
