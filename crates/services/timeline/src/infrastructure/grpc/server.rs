use std::net::SocketAddr;
use std::sync::Arc;

use cqrs::query::{InMemoryQueryBus, QueryBusBuilder};
use tonic::transport::{Channel, Server};
use tonic_health::server::health_reporter;
use tonic_reflection::server::Builder as ReflectionBuilder;

use redis_storage::{RedisClientBuilder, RedisConfig};
use scylla_storage::{ScyllaConfig, ScyllaSessionBuilder};
use transport::kafka::config::client::KafkaClientConfig;

use crate::application::command::{
    backfill_follow::BackfillFollowHandler,
    ingest_audio_index::IngestAudioIndexHandler,
    ingest_post_published::IngestPostPublishedHandler,
    prune_follow::PruneFollowHandler,
    remove_post::RemovePostHandler,
};
use crate::application::query::get_audio_feed::GetAudioFeedHandler;
use crate::application::query::get_following_feed::GetFollowingFeedHandler;
use crate::config::TimelineConfig;
use crate::infrastructure::cache::{
    RedisAudioFeedStore, RedisFeedStore, RedisFollowingStore, RedisTierCache, RedisVipRegistry,
};
use crate::infrastructure::client::SocialGraphGrpcClient;
use crate::infrastructure::grpc::handler::{TimelineServiceHandler, TimelineServiceServer};
use crate::infrastructure::persistence::{
    ScyllaAudioFeedRepository, ScyllaAuthorPostRepository, ScyllaFeedRepository,
};
use crate::infrastructure::worker::{
    follow_created_worker::FollowCreatedWorker,
    follow_deleted_worker::FollowDeletedWorker,
    post_deleted_worker::PostDeletedWorker,
    post_published_worker::PostPublishedWorker,
};

use cqrs::command::CommandBusBuilder;

/// Proto file descriptor blob embedded at build time for server reflection.
pub const FILE_DESCRIPTOR_SET: &[u8] =
    tonic::include_file_descriptor_set!("timeline_descriptor");

/// Bootstraps and runs the timeline gRPC server with all background workers.
pub async fn serve(addr: SocketAddr) -> Result<(), Box<dyn std::error::Error>> {
    let config = Arc::new(TimelineConfig::from_env());

    // ── Storage clients ───────────────────────────────────────────────────────

    let scylla_config = ScyllaConfig::from_env();
    let scylla_client = Arc::new(ScyllaSessionBuilder::new(scylla_config).build().await?);

    let redis_config = RedisConfig::from_env();
    let redis_client = RedisClientBuilder::new(redis_config).build().await?;

    // ── Infrastructure objects ────────────────────────────────────────────────

    let feed_store      = Arc::new(RedisFeedStore::new(redis_client.clone()));
    let vip_registry    = Arc::new(RedisVipRegistry::new(redis_client.clone()));
    let tier_cache      = Arc::new(RedisTierCache::new(redis_client.clone()));
    let following_store = Arc::new(RedisFollowingStore::new(redis_client.clone()));
    let audio_feed_store = Arc::new(RedisAudioFeedStore::new(redis_client));

    let feed_repository   = Arc::new(ScyllaFeedRepository::new(Arc::clone(&scylla_client)));
    let author_post_repo  = Arc::new(ScyllaAuthorPostRepository::new(Arc::clone(&scylla_client)));
    let audio_feed_repo   = Arc::new(ScyllaAudioFeedRepository::new(Arc::clone(&scylla_client)));

    // ── Social-graph gRPC client ──────────────────────────────────────────────

    let sg_channel = Channel::from_shared(config.social_graph_endpoint.clone())?
        .connect()
        .await?;
    let social_graph = Arc::new(SocialGraphGrpcClient::new(sg_channel));

    // ── Kafka config ──────────────────────────────────────────────────────────

    let kafka_config = KafkaClientConfig::from_env();

    // ── CQRS buses ────────────────────────────────────────────────────────────

    let command_bus = Arc::new(
        CommandBusBuilder::new()
            .register::<crate::application::command::ingest_post_published::IngestPostPublishedCommand, _>(
                IngestPostPublishedHandler {
                    feed_store:             Arc::clone(&feed_store),
                    vip_registry:           Arc::clone(&vip_registry),
                    feed_repository:        Arc::clone(&feed_repository),
                    author_post_repo:       Arc::clone(&author_post_repo),
                    tier_cache:             Arc::clone(&tier_cache),
                    social_graph:           Arc::clone(&social_graph),
                    audio_feed_repo:        Arc::clone(&audio_feed_repo),
                    audio_feed_store:       Arc::clone(&audio_feed_store),
                    feed_cap:               config.feed_cap,
                    vip_registry_cap:       config.vip_registry_cap,
                    vip_registry_ttl_secs:  config.vip_registry_ttl_secs,
                    tier_cache_ttl_secs:    config.tier_cache_ttl_secs,
                    social_graph_page_size: config.social_graph_page_size,
                    audio_feed_cap:         config.audio_feed_cap,
                },
            )?
            .register::<crate::application::command::remove_post::RemovePostCommand, _>(
                RemovePostHandler {
                    feed_store:       Arc::clone(&feed_store),
                    vip_registry:     Arc::clone(&vip_registry),
                    feed_repository:  Arc::clone(&feed_repository),
                    author_post_repo: Arc::clone(&author_post_repo),
                    tier_cache:       Arc::clone(&tier_cache),
                },
            )?
            .register::<crate::application::command::backfill_follow::BackfillFollowCommand, _>(
                BackfillFollowHandler {
                    feed_store:       Arc::clone(&feed_store),
                    feed_repository:  Arc::clone(&feed_repository),
                    author_post_repo: Arc::clone(&author_post_repo),
                    tier_cache:       Arc::clone(&tier_cache),
                    following_store:  Arc::clone(&following_store),
                    feed_cap:         config.feed_cap,
                    backfill_limit:   config.backfill_limit,
                },
            )?
            .register::<crate::application::command::prune_follow::PruneFollowCommand, _>(
                PruneFollowHandler {
                    feed_store:       Arc::clone(&feed_store),
                    feed_repository:  Arc::clone(&feed_repository),
                    tier_cache:       Arc::clone(&tier_cache),
                    following_store:  Arc::clone(&following_store),
                },
            )?
            .register::<crate::application::command::ingest_audio_index::IngestAudioIndexCommand, _>(
                IngestAudioIndexHandler {
                    audio_feed_repo:  Arc::clone(&audio_feed_repo),
                    audio_feed_store: Arc::clone(&audio_feed_store),
                    audio_feed_cap:   config.audio_feed_cap,
                },
            )?
            .build(),
    );

    let query_bus = QueryBusBuilder::new()
        .register::<crate::application::query::get_following_feed::GetFollowingFeedQuery, _>(
            GetFollowingFeedHandler {
                feed_store:             Arc::clone(&feed_store),
                vip_registry:           Arc::clone(&vip_registry),
                feed_repository:        Arc::clone(&feed_repository),
                author_post_repo:       Arc::clone(&author_post_repo),
                tier_cache:             Arc::clone(&tier_cache),
                following_store:        Arc::clone(&following_store),
                social_graph:           Arc::clone(&social_graph),
                max_page_size:          config.max_page_size,
                feed_cap:               config.feed_cap,
                vip_registry_cap:       config.vip_registry_cap,
                vip_registry_ttl_secs:  config.vip_registry_ttl_secs,
                warm_ttl_secs:          config.warm_ttl_secs,
                social_graph_page_size: config.social_graph_page_size,
                max_vip_merge_sources:  config.max_vip_merge_sources,
            },
        )?
        .register::<crate::application::query::get_audio_feed::GetAudioFeedQuery, _>(
            GetAudioFeedHandler {
                audio_feed_store: Arc::clone(&audio_feed_store),
                audio_feed_repo:  Arc::clone(&audio_feed_repo),
                max_page_size:    config.max_page_size,
            },
        )?
        .build();

    // ── Background workers ────────────────────────────────────────────────────

    tokio::spawn(
        PostPublishedWorker::new(
            kafka_config.clone(),
            Arc::clone(&command_bus),
            config.kafka_group_post_published.clone(),
        )
        .run(),
    );

    tokio::spawn(
        PostDeletedWorker::new(
            kafka_config.clone(),
            Arc::clone(&command_bus),
            config.kafka_group_post_deleted.clone(),
        )
        .run(),
    );

    tokio::spawn(
        FollowCreatedWorker::new(
            kafka_config.clone(),
            Arc::clone(&command_bus),
            config.kafka_group_sg_followed.clone(),
        )
        .run(),
    );

    tokio::spawn(
        FollowDeletedWorker::new(
            kafka_config,
            Arc::clone(&command_bus),
            config.kafka_group_sg_unfollowed.clone(),
        )
        .run(),
    );

    // ── gRPC server ───────────────────────────────────────────────────────────

    let (health_reporter, health_service) = health_reporter();
    health_reporter
        .set_serving::<TimelineServiceServer<
            TimelineServiceHandler<InMemoryQueryBus>,
        >>()
        .await;

    let reflection = ReflectionBuilder::configure()
        .register_encoded_file_descriptor_set(FILE_DESCRIPTOR_SET)
        .build_v1()?;

    let svc = TimelineServiceServer::new(TimelineServiceHandler::new(query_bus));

    tracing::info!(addr = %addr, "timeline gRPC server listening");

    Server::builder()
        .add_service(health_service)
        .add_service(reflection)
        .add_service(svc)
        .serve(addr)
        .await?;

    Ok(())
}
