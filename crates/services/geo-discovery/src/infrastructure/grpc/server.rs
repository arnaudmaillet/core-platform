use std::net::SocketAddr;
use std::sync::Arc;

use cqrs::query::{InMemoryQueryBus, QueryBusBuilder};
use tonic::transport::Server;
use tonic_health::server::health_reporter;
use tonic_reflection::server::Builder as ReflectionBuilder;

use redis_storage::{RedisClientBuilder, RedisConfig};
use scylla_storage::{ScyllaConfig, ScyllaSessionBuilder};
use transport::kafka::config::client::KafkaClientConfig;

use crate::application::query::query_tile::QueryTileHandler;
use crate::config::GeoDiscoveryConfig;
use crate::infrastructure::cache::{RedisCardStore, RedisGeoSpatialIndex};
use crate::infrastructure::grpc::handler::{GeoDiscoveryHandler, GeoDiscoveryServiceServer};
use crate::infrastructure::persistence::ScyllaTileRepository;
use crate::infrastructure::worker::{PostIndexerWorker, ScoreUpdaterWorker, TilePrunerWorker};

/// Proto file descriptor set embedded at build time for server reflection.
pub const FILE_DESCRIPTOR_SET: &[u8] =
    tonic::include_file_descriptor_set!("geo_discovery_descriptor");

/// Bootstraps and runs the geo-discovery gRPC server.
///
/// Initialises all storage clients, assembles the CQRS buses, spawns background
/// workers, and blocks until the server shuts down.
pub async fn serve(addr: SocketAddr) -> Result<(), Box<dyn std::error::Error>> {
    let cfg = GeoDiscoveryConfig::from_env();

    // ── Storage clients ───────────────────────────────────────────────────────

    let scylla_config = ScyllaConfig::from_env();
    let scylla_client = Arc::new(ScyllaSessionBuilder::new(scylla_config).build().await?);

    let redis_config = RedisConfig::from_env();
    let redis_client = RedisClientBuilder::new(redis_config).build().await?;

    // ── Infrastructure objects ────────────────────────────────────────────────

    let spatial_index   = Arc::new(RedisGeoSpatialIndex::new(redis_client.clone()));
    let card_store      = Arc::new(RedisCardStore::new(redis_client.clone()));
    let tile_repository = Arc::new(ScyllaTileRepository::new(Arc::clone(&scylla_client)));

    // ── CQRS query bus ────────────────────────────────────────────────────────

    let query_bus = QueryBusBuilder::new()
        .register::<crate::application::query::query_tile::QueryTileQuery, _>(
            QueryTileHandler {
                spatial_index:   Arc::clone(&spatial_index),
                card_store:      Arc::clone(&card_store),
                tile_repository: Arc::clone(&tile_repository),
            },
        )?
        .build();

    // ── Background workers ────────────────────────────────────────────────────

    let kafka_config = KafkaClientConfig::from_env();

    let post_indexer = PostIndexerWorker::new(
        kafka_config.clone(),
        Arc::clone(&spatial_index),
        Arc::clone(&card_store),
        Arc::clone(&tile_repository),
        cfg.post_indexer_group_id.clone(),
        cfg.card_cache_threshold,
    );
    tokio::spawn(post_indexer.run());

    let score_updater = ScoreUpdaterWorker::new(
        kafka_config,
        Arc::clone(&spatial_index),
        Arc::clone(&tile_repository),
        cfg.score_updater_group_id.clone(),
    );
    tokio::spawn(score_updater.run());

    let tile_pruner = TilePrunerWorker::new(
        redis_client,
        cfg.tile_pruner_interval,
        cfg.tile_cold_threshold,
        500,
    );
    tokio::spawn(tile_pruner.run());

    // ── gRPC server ───────────────────────────────────────────────────────────

    let (health_reporter, health_service) = health_reporter();
    health_reporter
        .set_serving::<GeoDiscoveryServiceServer<GeoDiscoveryHandler<InMemoryQueryBus>>>()
        .await;

    let reflection = ReflectionBuilder::configure()
        .register_encoded_file_descriptor_set(FILE_DESCRIPTOR_SET)
        .build_v1()?;

    let svc = GeoDiscoveryServiceServer::new(GeoDiscoveryHandler::new(query_bus));

    tracing::info!(addr = %addr, "geo-discovery gRPC server listening");

    Server::builder()
        .add_service(health_service)
        .add_service(reflection)
        .add_service(svc)
        .serve(addr)
        .await?;

    Ok(())
}
