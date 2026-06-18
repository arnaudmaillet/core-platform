// crates/geo_discovery/src/application/fixture.rs

use geo_discovery::GeoDiscoveryServiceBuilder;
use shared_kernel::command::CommandBus;
use shared_kernel::environment::ClusterContext;
use shared_kernel::types::{ProfileId, Region};
use shared_kernel_test_utils::repositories::{CacheRepositoryStub, IdempotencyRepositoryStub};
use std::sync::Arc;

use crate::repositories::{MapCacheRepositoryStub, MapRepositoryStub};
use crate::resolvers::EngagementResolverStub;
use geo_discovery::context::{GeoDiscoveryCommandCtx, GeoDiscoveryKernelCtx, GeoDiscoveryQueryCtx};

pub struct GeoDiscoveryTestFixture {
    bus: CommandBus,
    operator_id: ProfileId,

    kernel_ctx: GeoDiscoveryKernelCtx,
    command_ctx: GeoDiscoveryCommandCtx,
    query_ctx: GeoDiscoveryQueryCtx,
    cluster_ctx: ClusterContext,

    map_repo: Arc<MapRepositoryStub>,
    map_cache_repo: Arc<MapCacheRepositoryStub>,
    idempotency_repo: Arc<IdempotencyRepositoryStub>,
}

impl GeoDiscoveryTestFixture {
    pub async fn new() -> Self {
        let map_repo = Arc::new(MapRepositoryStub::new());
        let map_cache_repo = Arc::new(MapCacheRepositoryStub::new());
        let idempotency_repo = Arc::new(IdempotencyRepositoryStub::new());
        let engagement_resolver = Arc::new(EngagementResolverStub);
        let cache_repo = Arc::new(CacheRepositoryStub::new());

        let max_posts_per_tile = 50;
        let operator_id = ProfileId::generate();
        let cluster_ctx = ClusterContext::default();

        let mut bus = CommandBus::new(Some(idempotency_repo.clone()), Some(cache_repo));

        let service = GeoDiscoveryServiceBuilder::new(
            map_repo.clone(),
            map_cache_repo.clone(),
            idempotency_repo.clone(),
            engagement_resolver.clone(),
            max_posts_per_tile,
            cluster_ctx.clone(),
        );

        let kernel_ctx = service.build_context().await;
        let command_ctx =
            GeoDiscoveryCommandCtx::new(kernel_ctx.clone(), operator_id, cluster_ctx.region());
        let query_ctx = GeoDiscoveryQueryCtx::new(kernel_ctx.clone(), cluster_ctx.region());

        service.register_handlers(&mut bus);

        Self {
            bus,
            operator_id,
            kernel_ctx,
            command_ctx,
            query_ctx,
            cluster_ctx,
            map_repo,
            map_cache_repo,
            idempotency_repo,
        }
    }

    pub fn bus(&self) -> &CommandBus {
        &self.bus
    }

    pub fn region(&self) -> Region {
        self.cluster_ctx.region()
    }

    pub fn operator_id(&self) -> ProfileId {
        self.operator_id
    }

    pub fn kernel_ctx(&self) -> &GeoDiscoveryKernelCtx {
        &self.kernel_ctx
    }

    pub fn command_ctx(&self) -> &GeoDiscoveryCommandCtx {
        &self.command_ctx
    }

    pub fn query_ctx(&self) -> &GeoDiscoveryQueryCtx {
        &self.query_ctx
    }

    pub fn map_repo(&self) -> &MapRepositoryStub {
        &self.map_repo
    }

    pub fn map_cache_repo(&self) -> &MapCacheRepositoryStub {
        &self.map_cache_repo
    }

    pub fn idempotency_repo(&self) -> &IdempotencyRepositoryStub {
        &self.idempotency_repo
    }
}
