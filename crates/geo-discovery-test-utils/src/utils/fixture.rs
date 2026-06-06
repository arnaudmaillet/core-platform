// crates/geo_discovery/src/application/fixture.rs

use geo_discovery::repositories::{MapCacheRepository, MapPersistenceRepository};
use shared_kernel::command::CommandBus;
use shared_kernel::types::{PostId, Region};
use shared_kernel_test_utils::repositories::{CacheRepositoryStub, IdempotencyRepositoryStub};
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::repositories::{StubMapCacheRepository, StubMapPersistenceRepository};
use crate::resolvers::EngagementResolverStub;
use geo_discovery::context::{
    GeoDiscoveryAppContext, GeoDiscoveryCommandContext, GeoDiscoveryQueryContext,
};
use geo_discovery::handlers::{HydrateTileCacheCommand, HydrateTileCacheHandler};
use geo_discovery::handlers::{IndexActivePostCommand, IndexActivePostHandler};
use geo_discovery::handlers::{RemovePostFromMapCommand, RemovePostFromMapHandler};
use geo_discovery::types::{BucketHour, TileH3, TileResolution};

#[allow(dead_code)]
pub struct GeoDiscoveryTestFixture {
    bus: CommandBus,
    region: Region,
    app_ctx: GeoDiscoveryAppContext,
    command_ctx: GeoDiscoveryCommandContext,
    query_ctx: GeoDiscoveryQueryContext,
    persistence_repo: Arc<StubMapPersistenceRepository>,
    cache_repo: Arc<StubMapCacheRepository>,
    idempotency_repo: Arc<IdempotencyRepositoryStub>,
    hydration_sender: mpsc::Sender<HydrateTileCacheCommand>,
}

impl Default for GeoDiscoveryTestFixture {
    fn default() -> Self {
        Self::new()
    }
}

impl GeoDiscoveryTestFixture {
    pub fn new() -> Self {
        let persistence_repo = Arc::new(StubMapPersistenceRepository::new());
        let cache_repo = Arc::new(StubMapCacheRepository::new());
        let idempotency_repo = Arc::new(IdempotencyRepositoryStub::new());
        let engagement_resolver = Arc::new(EngagementResolverStub);
        let shared_bus_cache = Arc::new(CacheRepositoryStub::new());

        let max_posts_per_tile = 50;
        let (hydration_sender, mut hydration_receiver) = mpsc::channel(100);

        let app_ctx = GeoDiscoveryAppContext::new(
            persistence_repo.clone(),
            cache_repo.clone(),
            idempotency_repo.clone(),
            engagement_resolver,
            hydration_sender.clone(),
        );

        let region = Region::default();
        let operator_id = shared_kernel::types::ProfileId::generate();

        let command_ctx = app_ctx.command(operator_id, region);
        let query_ctx = app_ctx.query(region);

        let mut bus = CommandBus::new(shared_bus_cache);

        bus.register::<GeoDiscoveryCommandContext, IndexActivePostCommand, IndexActivePostHandler>(
            IndexActivePostHandler,
        );

        bus.register::<GeoDiscoveryCommandContext, RemovePostFromMapCommand, RemovePostFromMapHandler>(
            RemovePostFromMapHandler,
        );

        let hydration_handler = Arc::new(HydrateTileCacheHandler::new(
            cache_repo.clone(),
            persistence_repo.clone(),
            max_posts_per_tile,
        ));

        let hydration_handler_worker = hydration_handler.clone();
        tokio::spawn(async move {
            while let Some(cmd) = hydration_receiver.recv().await {
                let _ = hydration_handler_worker.handle(cmd).await;
            }
        });

        Self {
            bus,
            region,
            app_ctx,
            command_ctx,
            query_ctx,
            persistence_repo,
            cache_repo,
            idempotency_repo,
            hydration_sender,
        }
    }

    pub fn bus(&self) -> &CommandBus {
        &self.bus
    }
    pub fn region(&self) -> Region {
        self.region
    }
    pub fn command_ctx(&self) -> &GeoDiscoveryCommandContext {
        &self.command_ctx
    }
    pub fn query_ctx(&self) -> &GeoDiscoveryQueryContext {
        &self.query_ctx
    }
    pub fn persistence_repo(&self) -> &StubMapPersistenceRepository {
        &self.persistence_repo
    }
    pub fn cache_repo(&self) -> &StubMapCacheRepository {
        &self.cache_repo
    }
    pub fn idempotency_repo(&self) -> &IdempotencyRepositoryStub {
        &self.idempotency_repo
    }

    pub fn cache_repo_dyn(&self) -> Arc<dyn MapCacheRepository> {
        self.cache_repo.clone() as Arc<dyn MapCacheRepository>
    }

    pub fn persistence_repo_dyn(&self) -> Arc<dyn MapPersistenceRepository> {
        self.persistence_repo.clone() as Arc<dyn MapPersistenceRepository>
    }

    pub async fn assert_persisted_post_exists(
        &self,
        res: TileResolution,
        tile: &TileH3,
        bucket: BucketHour,
        post_id: &PostId,
    ) {
        let records = self
            .persistence_repo
            .find_by_tile(res, tile, bucket)
            .await
            .expect("Scylla stub read failure");
        let found = records.iter().any(|p| p.post_id() == *post_id);
        assert!(
            found,
            "Post {} missing from persistence for tile {:?}",
            post_id, tile
        );
    }

    pub async fn assert_cache_post_count(
        &self,
        res: TileResolution,
        tile: &TileH3,
        expected: usize,
    ) {
        let count = self
            .cache_repo
            .get_tile_post_count(res, tile)
            .await
            .expect("Redis stub read failure");
        assert_eq!(count, expected, "Cache count mismatch for tile {:?}", tile);
    }
}
