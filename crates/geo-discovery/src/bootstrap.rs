// crates/geo_discovery/src/application/builder.rs

use infra_fred::fred::clients::Pool;
use infra_scylla::scylla::client::session::Session as ScyllaSession;
use shared_kernel::idempotency::IdempotencyRepository;
use std::sync::Arc;

use crate::context::GeoDiscoveryAppContext;
use crate::db::{FredMapCacheRepository, ScyllaMapPersistenceRepository};
use crate::repositories::{MapCacheRepository, MapPersistenceRepository};

pub struct GeoDiscoveryServiceBuilder {
    scylla_session: Arc<ScyllaSession>,
    fred_pool: Pool,
    idempotency_repo: Arc<dyn IdempotencyRepository>,
}

impl GeoDiscoveryServiceBuilder {
    pub fn new(
        scylla_session: Arc<ScyllaSession>,
        fred_pool: Pool,
        idempotency_repo: Arc<dyn IdempotencyRepository>,
    ) -> Self {
        Self {
            scylla_session,
            fred_pool,
            idempotency_repo,
        }
    }

    pub async fn build_context(
        &self,
    ) -> Result<Arc<GeoDiscoveryAppContext>, infra_scylla::scylla::errors::PrepareError> {
        let persistence_repo: Arc<dyn MapPersistenceRepository> =
            Arc::new(ScyllaMapPersistenceRepository::new(self.scylla_session.clone()).await?);

        let cache_repo: Arc<dyn MapCacheRepository> =
            Arc::new(FredMapCacheRepository::new(self.fred_pool.clone()));

        Ok(Arc::new(GeoDiscoveryAppContext::new(
            persistence_repo,
            cache_repo,
            self.idempotency_repo.clone(),
        )))
    }
}
