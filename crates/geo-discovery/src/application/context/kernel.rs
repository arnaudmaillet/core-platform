// crates/geo_discovery/src/application/context/app.rs

use shared_kernel::environment::ClusterContext;
use shared_kernel::idempotency::IdempotencyRepository;
use shared_kernel::types::Region;
use std::sync::Arc;
use tokio::sync::mpsc::Sender;

use crate::repositories::{MapAnnotationDiscoveryRepository, MapAnnotationArchiveRepository};
use crate::resolvers::EngagementResolver;
use crate::use_cases::HydrateTileCacheCommand;

#[derive(Clone)]
pub struct GeoDiscoveryKernelCtx {
    annotation_repo: Arc<dyn MapAnnotationArchiveRepository>,
    annotation_cache_repo: Arc<dyn MapAnnotationDiscoveryRepository>,
    idempotency_repo: Arc<dyn IdempotencyRepository>,
    engagement_resolver: Arc<dyn EngagementResolver>,
    hydration_sender: Sender<HydrateTileCacheCommand>,
    cluster_ctx: ClusterContext,
}

impl GeoDiscoveryKernelCtx {
    pub fn new(
        annotation_repo: Arc<dyn MapAnnotationArchiveRepository>,
        annotation_cache_repo: Arc<dyn MapAnnotationDiscoveryRepository>,
        idempotency_repo: Arc<dyn IdempotencyRepository>,
        engagement_resolver: Arc<dyn EngagementResolver>,
        hydration_sender: Sender<HydrateTileCacheCommand>,
        cluster_ctx: ClusterContext,
    ) -> Self {
        Self {
            annotation_repo,
            annotation_cache_repo,
            idempotency_repo,
            engagement_resolver,
            hydration_sender,
            cluster_ctx,
        }
    }

    pub fn server_region(&self) -> Region {
        self.cluster_ctx.region()
    }

    pub fn storage_repo(&self) -> Arc<dyn MapAnnotationArchiveRepository> {
        self.annotation_repo.clone()
    }

    pub fn cache_repo(&self) -> Arc<dyn MapAnnotationDiscoveryRepository> {
        self.annotation_cache_repo.clone()
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
