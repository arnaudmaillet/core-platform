use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use cqrs::command::{CommandBusBuilder, InMemoryCommandBus};
use cqrs::query::{QueryBusBuilder, InMemoryQueryBus};
use redis_storage::{RedisClientBuilder, RedisConfig};
use scylla_storage::{ScyllaConfig, ScyllaSessionBuilder};
use tonic::transport::Server;
use tonic_health::server::health_reporter;
use tonic_reflection::server::Builder as ReflectionBuilder;
use transport::kafka::config::client::KafkaClientConfig;

use crate::application::command::{
    create_notification::CreateNotificationHandler,
    mark_read::{MarkAllReadHandler, MarkReadHandler},
};
use crate::application::command::create_notification::CreateNotificationCommand;
use crate::application::command::mark_read::{MarkAllReadCommand, MarkReadCommand};
use crate::application::query::{
    get_unread_count::{GetUnreadCountHandler, GetUnreadCountQuery},
    list_notifications::{ListNotificationsHandler, ListNotificationsQuery},
};
use crate::config::NotificationConfig;
use crate::infrastructure::cache::{RedisBlockCache, RedisUnreadCounter};
use crate::infrastructure::grpc::handler::notification_handler::{
    NotificationServiceHandler, NotificationServiceServer,
};
use crate::infrastructure::persistence::ScyllaNotificationRepository;
use crate::infrastructure::streaming::BroadcastRegistry;
use crate::infrastructure::worker::{
    collapse_flush_worker::CollapseFlushWorker,
    comment_worker::CommentNotificationWorker,
    mention_worker::MentionNotificationWorker,
    reaction_worker::ReactionNotificationWorker,
};

pub const FILE_DESCRIPTOR_SET: &[u8] =
    tonic::include_file_descriptor_set!("notification_descriptor");

/// Bootstraps and runs the notification gRPC server.
///
/// Initialises all storage clients, injects dependencies via constructor
/// injection, spawns background Kafka workers, and blocks until the server
/// shuts down or returns an error.
pub async fn serve(addr: SocketAddr) -> Result<(), Box<dyn std::error::Error>> {
    // ── Configuration ─────────────────────────────────────────────────────────

    let config = Arc::new(NotificationConfig::from_env());

    // ── Storage clients ───────────────────────────────────────────────────────

    let scylla_config = ScyllaConfig::from_env();
    let scylla_client = Arc::new(ScyllaSessionBuilder::new(scylla_config).build().await?);

    let redis_config = RedisConfig::from_env();
    let redis_client = RedisClientBuilder::new(redis_config).build().await?;

    // ── Infrastructure objects ────────────────────────────────────────────────

    let repository = Arc::new(ScyllaNotificationRepository::new(Arc::clone(&scylla_client)));

    let block_cache = Arc::new(RedisBlockCache::new(
        redis_client.clone(),
        Arc::clone(&config),
    ));

    let counter = Arc::new(RedisUnreadCounter::new(
        redis_client.clone(),
        Arc::clone(&repository),
        Arc::clone(&config),
    ));

    let stream_registry = Arc::new(BroadcastRegistry::new(config.stream_buffer_size));

    // ── CQRS buses ────────────────────────────────────────────────────────────

    let command_bus = CommandBusBuilder::new()
        .register::<CreateNotificationCommand, _>(CreateNotificationHandler {
            repository:      Arc::clone(&repository),
            block_cache:     Arc::clone(&block_cache),
            counter:         Arc::clone(&counter),
            stream_registry: Arc::clone(&stream_registry),
        })?
        .register::<MarkReadCommand, _>(MarkReadHandler {
            repository: Arc::clone(&repository),
            counter:    Arc::clone(&counter),
        })?
        .register::<MarkAllReadCommand, _>(MarkAllReadHandler {
            repository: Arc::clone(&repository),
            counter:    Arc::clone(&counter),
        })?
        .build();

    let query_bus = QueryBusBuilder::new()
        .register::<ListNotificationsQuery, _>(ListNotificationsHandler {
            repository:    Arc::clone(&repository),
            counter:       Arc::clone(&counter),
            max_page_size: config.max_page_size,
        })?
        .register::<GetUnreadCountQuery, _>(GetUnreadCountHandler {
            counter: Arc::clone(&counter),
        })?
        .build();

    // ── Kafka config ──────────────────────────────────────────────────────────

    let kafka_config = KafkaClientConfig::from_env();

    // ── Background workers ────────────────────────────────────────────────────

    let reaction_worker = ReactionNotificationWorker::new(
        kafka_config.clone(),
        redis_client.clone(),
        Arc::clone(&repository),
        Arc::clone(&block_cache),
        Arc::clone(&counter),
        Arc::clone(&stream_registry),
        Arc::clone(&config),
        "notification-reaction-consumer",
    );
    tokio::spawn(reaction_worker.run());

    let comment_worker = CommentNotificationWorker::new(
        kafka_config.clone(),
        redis_client.clone(),
        Arc::clone(&repository),
        Arc::clone(&block_cache),
        Arc::clone(&counter),
        Arc::clone(&stream_registry),
        Arc::clone(&config),
        "notification-comment-consumer",
    );
    tokio::spawn(comment_worker.run());

    let mention_worker = MentionNotificationWorker::new(
        kafka_config.clone(),
        redis_client.clone(),
        Arc::clone(&repository),
        Arc::clone(&block_cache),
        Arc::clone(&counter),
        Arc::clone(&stream_registry),
        Arc::clone(&config),
        "notification-mention-consumer",
    );
    tokio::spawn(mention_worker.run());

    let flush_worker = CollapseFlushWorker::new(
        redis_client.clone(),
        Arc::clone(&repository),
        Arc::clone(&counter),
        Arc::clone(&stream_registry),
        Arc::clone(&config),
        Duration::from_secs(config.collapse_flush_interval_secs),
    );
    tokio::spawn(flush_worker.run());

    // Periodic reaper for the broadcast registry — without it the per-profile
    // sender map grows unbounded (one entry per profile that ever connected).
    tokio::spawn(Arc::clone(&stream_registry).run_reaper());

    // ── gRPC server ───────────────────────────────────────────────────────────

    let (health_reporter, health_service) = health_reporter();
    health_reporter
        .set_serving::<NotificationServiceServer<
            NotificationServiceHandler<InMemoryCommandBus, InMemoryQueryBus, BroadcastRegistry>,
        >>()
        .await;

    let reflection = ReflectionBuilder::configure()
        .register_encoded_file_descriptor_set(FILE_DESCRIPTOR_SET)
        .build_v1()?;

    let svc = NotificationServiceServer::new(NotificationServiceHandler::new(
        command_bus,
        query_bus,
        Arc::clone(&stream_registry),
    ));

    tracing::info!(addr = %addr, "notification gRPC server listening");

    Server::builder()
        .add_service(health_service)
        .add_service(reflection)
        .add_service(svc)
        .serve(addr)
        .await?;

    Ok(())
}
