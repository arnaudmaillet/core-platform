// crates/geo_discovery/src/application/builder.rs

use shared_kernel::command::CommandBus;
use shared_kernel::environment::ClusterContext;
use shared_kernel::idempotency::IdempotencyRepository;
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::context::{GeoDiscoveryCommandCtx, GeoDiscoveryKernelCtx};
use crate::repositories::{MapAnnotationArchiveRepository, MapAnnotationDiscoveryRepository};
use crate::resolvers::EngagementResolver;
use crate::use_cases::{
    HydrateTileCacheCommand, HydrateTileCacheHandler, IndexMapAnnotationCommand,
    IndexMapAnnotationHandler, RemoveMapAnnotationCommand, RemoveMapAnnotationHandler,
};
use crate::workers::MapAnnotationCacheHydrationWorker;

pub struct GeoDiscoveryServiceBuilder {
    archive_repo: Arc<dyn MapAnnotationArchiveRepository>,
    discovery_repo: Arc<dyn MapAnnotationDiscoveryRepository>,
    idempotency_repo: Arc<dyn IdempotencyRepository>,
    engagement_resolver: Arc<dyn EngagementResolver>,
    max_posts_per_tile: usize,
    cluster_ctx: ClusterContext,
}

impl GeoDiscoveryServiceBuilder {
    pub fn new(
        archive_repo: Arc<dyn MapAnnotationArchiveRepository>,
        discovery_repo: Arc<dyn MapAnnotationDiscoveryRepository>,
        idempotency_repo: Arc<dyn IdempotencyRepository>,
        engagement_resolver: Arc<dyn EngagementResolver>,
        max_posts_per_tile: usize,
        cluster_ctx: ClusterContext,
    ) -> Self {
        Self {
            archive_repo,
            discovery_repo,
            idempotency_repo,
            engagement_resolver,
            max_posts_per_tile,
            cluster_ctx,
        }
    }

    pub async fn build_context(&self) -> GeoDiscoveryKernelCtx {
        let (hydration_sender, hydration_receiver) =
            mpsc::channel::<HydrateTileCacheCommand>(10000);

        let hydration_handler = HydrateTileCacheHandler::new(
            self.discovery_repo.clone(),
            self.archive_repo.clone(),
            self.max_posts_per_tile,
        );

        let hydration_worker = MapAnnotationCacheHydrationWorker::new(hydration_handler);
        hydration_worker.start(hydration_receiver);

        GeoDiscoveryKernelCtx::new(
            self.archive_repo.clone(),
            self.discovery_repo.clone(),
            self.idempotency_repo.clone(),
            self.engagement_resolver.clone(),
            hydration_sender,
            self.cluster_ctx,
        )
    }

    pub fn register_handlers(&self, bus: &mut CommandBus) {
        bus.register::<GeoDiscoveryCommandCtx, IndexMapAnnotationCommand, IndexMapAnnotationHandler>(
            IndexMapAnnotationHandler,
        );
        bus.register::<GeoDiscoveryCommandCtx, RemoveMapAnnotationCommand, RemoveMapAnnotationHandler>(
            RemoveMapAnnotationHandler,
        );
    }
}
