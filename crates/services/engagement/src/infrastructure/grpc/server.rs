use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use cqrs::command::{CommandBusBuilder, InMemoryCommandBus};
use cqrs::query::{QueryBusBuilder, InMemoryQueryBus};
use tonic::transport::Server;
use tonic_health::server::health_reporter;
use tonic_reflection::server::Builder as ReflectionBuilder;

use redis_storage::{RedisClientBuilder, RedisConfig};
use scylla_storage::{ScyllaConfig, ScyllaSessionBuilder};
use transport::kafka::config::client::KafkaClientConfig;
use transport::kafka::producer::builder::KafkaProducerBuilder;
use transport::kafka::config::producer::ProducerConfig;

use crate::application::command::{
    record_share::RecordShareHandler,
    record_view::RecordViewHandler,
    remove_reaction::RemoveReactionHandler,
    upsert_reaction::UpsertReactionHandler,
};
use crate::application::query::get_post_engagement::GetPostEngagementHandler;
use crate::config::ReactionWeightsConfig;
use crate::infrastructure::grpc::handler::engagement_handler::{
    EngagementServiceHandler, EngagementServiceServer,
};
use crate::infrastructure::persistence::ScyllaReactionLedger;
use crate::infrastructure::publisher::KafkaEngagementEventPublisher;
use crate::infrastructure::scoring::redis_score_store::{DirtyPostTracker, RedisScoreStore};
use crate::infrastructure::worker::{
    comment_consumer::CommentEventConsumer,
    counter_flush::CounterFlushWorker,
    reaction_write_behind::ReactionWriteBehindWorker,
};

/// Proto file descriptor blob embedded at build time for server reflection.
pub const FILE_DESCRIPTOR_SET: &[u8] =
    tonic::include_file_descriptor_set!("engagement_descriptor");

/// Bootstraps and runs the engagement gRPC server.
///
/// Initialises all storage clients, injects dependencies, registers background
/// workers, and blocks until the server shuts down.
pub async fn serve(addr: SocketAddr) -> Result<(), Box<dyn std::error::Error>> {
    // ── Storage clients ───────────────────────────────────────────────────────

    let scylla_config = ScyllaConfig::from_env();
    let scylla_client = Arc::new(ScyllaSessionBuilder::new(scylla_config).build().await?);

    let redis_config = RedisConfig::from_env();
    let redis_client = RedisClientBuilder::new(redis_config).build().await?;

    // ── Reaction weights ──────────────────────────────────────────────────────

    let weights = Arc::new(ReactionWeightsConfig::from_env()?);

    // ── Kafka producer ────────────────────────────────────────────────────────

    let kafka_client = KafkaClientConfig::from_env();
    let producer_config = ProducerConfig::new(kafka_client.clone());
    let producer = KafkaProducerBuilder::new(producer_config).build()?;

    // ── Infrastructure objects ────────────────────────────────────────────────

    let dirty_tracker = DirtyPostTracker::new();

    let score_store = Arc::new(RedisScoreStore::new(redis_client, dirty_tracker.clone()));
    let ledger      = Arc::new(ScyllaReactionLedger::new(Arc::clone(&scylla_client)));
    let publisher   = Arc::new(KafkaEngagementEventPublisher::new(producer));

    // ── CQRS buses ────────────────────────────────────────────────────────────

    let command_bus = CommandBusBuilder::new()
        .register::<crate::application::command::upsert_reaction::UpsertReactionCommand, _>(
            UpsertReactionHandler {
                score_store: Arc::clone(&score_store),
                publisher:   Arc::clone(&publisher),
                weights:     Arc::clone(&weights),
            },
        )?
        .register::<crate::application::command::remove_reaction::RemoveReactionCommand, _>(
            RemoveReactionHandler {
                score_store: Arc::clone(&score_store),
                publisher:   Arc::clone(&publisher),
            },
        )?
        .register::<crate::application::command::record_view::RecordViewCommand, _>(
            RecordViewHandler { score_store: Arc::clone(&score_store) },
        )?
        .register::<crate::application::command::record_share::RecordShareCommand, _>(
            RecordShareHandler { score_store: Arc::clone(&score_store) },
        )?
        .build();

    let query_bus = QueryBusBuilder::new()
        .register::<crate::application::query::get_post_engagement::GetPostEngagementQuery, _>(
            GetPostEngagementHandler { score_store: Arc::clone(&score_store) },
        )?
        .build();

    // ── Background workers ────────────────────────────────────────────────────

    let wb_worker = ReactionWriteBehindWorker::new(
        kafka_client.clone(),
        Arc::clone(&ledger),
        "engagement-reaction-write-behind",
    );
    tokio::spawn(wb_worker.run());

    let flush_worker = CounterFlushWorker::new(
        Arc::clone(&score_store),
        Arc::clone(&ledger),
        dirty_tracker,
        Duration::from_secs(5),
    );
    tokio::spawn(flush_worker.run());

    let comment_worker = CommentEventConsumer::new(
        kafka_client,
        Arc::clone(&score_store),
        Arc::clone(&ledger),
        "engagement-comment-consumer",
    );
    tokio::spawn(comment_worker.run());

    // ── gRPC server ───────────────────────────────────────────────────────────

    let (health_reporter, health_service) = health_reporter();
    health_reporter
        .set_serving::<EngagementServiceServer<EngagementServiceHandler<InMemoryCommandBus, InMemoryQueryBus>>>()
        .await;

    let reflection = ReflectionBuilder::configure()
        .register_encoded_file_descriptor_set(FILE_DESCRIPTOR_SET)
        .build_v1()?;

    let svc = EngagementServiceServer::new(EngagementServiceHandler::new(command_bus, query_bus));

    tracing::info!(addr = %addr, "engagement gRPC server listening");

    Server::builder()
        .add_service(health_service)
        .add_service(reflection)
        .add_service(svc)
        .serve(addr)
        .await?;

    Ok(())
}
