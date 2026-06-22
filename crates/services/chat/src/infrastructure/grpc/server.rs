use std::net::SocketAddr;
use std::sync::Arc;

use cqrs::command::{CommandBusBuilder, InMemoryCommandBus};
use cqrs::query::{InMemoryQueryBus, QueryBusBuilder};
use redis_storage::{RedisClientBuilder, RedisConfig, RedisSubscriberBuilder};
use scylla_storage::{ScyllaConfig, ScyllaSessionBuilder};
use tonic::transport::Server;
use tonic_health::server::health_reporter;
use tonic_reflection::server::Builder as ReflectionBuilder;
use transport::kafka::config::client::KafkaClientConfig;
use transport::kafka::config::producer::ProducerConfig;
use transport::kafka::producer::KafkaProducerBuilder;

use crate::application::command::{
    CreateConversationCommand, CreateConversationHandler, JoinAsMemberCommand, JoinAsMemberHandler,
    MarkReadCommand, MarkReadHandler, SendMessageCommand, SendMessageHandler, SubscribeCommand,
    SubscribeHandler, ToggleVisibilityCommand, ToggleVisibilityHandler, UnsubscribeCommand,
    UnsubscribeHandler,
};
use crate::application::query::{
    GetHistoryHandler, GetHistoryQuery, ListMembersHandler, ListMembersQuery,
    ListSubscriptionsHandler, ListSubscriptionsQuery,
};
use crate::application::port::{
    ConversationRepository, MemberRepository, PresenceStore, ReceiptStore, RoutingRegistry,
};
use crate::config::ChatConfig;
use crate::infrastructure::cache::{
    RedisHotTailCache, RedisPresenceStore, RedisReceiptStore, RedisRoutingRegistry,
};
use crate::infrastructure::event::KafkaEventPublisher;
use crate::infrastructure::grpc::handler::{ChatServiceHandler, ChatServiceServer};
use crate::infrastructure::grpc::handler::chat_handler::StreamingParams;
use crate::infrastructure::persistence::{
    ScyllaConversationRepository, ScyllaMemberRepository, ScyllaMessageRepository,
    ScyllaSubscriptionRepository,
};
use crate::infrastructure::routing::{Fanout, MessageFanout, PlaneAttach, PlaneSubscriber, RedisPlaneBroadcaster};
use crate::infrastructure::streaming::{ConversationBroadcastRegistry, PlaneFanoutSink};
use crate::infrastructure::worker::VisibilityWorker;

pub const FILE_DESCRIPTOR_SET: &[u8] = tonic::include_file_descriptor_set!("chat_descriptor");

/// Bootstraps and runs the chat gRPC server.
///
/// Wires storage, cache, and routing adapters; builds the CQRS buses; starts the
/// per-pod plane subscriber and the registry reapers; and serves until shutdown.
pub async fn serve(addr: SocketAddr) -> Result<(), Box<dyn std::error::Error>> {
    let config = Arc::new(ChatConfig::from_env());

    // ── Storage clients ───────────────────────────────────────────────────────

    let scylla_client = Arc::new(ScyllaSessionBuilder::new(ScyllaConfig::from_env()).build().await?);
    let redis_client = RedisClientBuilder::new(RedisConfig::from_env()).build().await?;
    let redis_subscriber = RedisSubscriberBuilder::new(RedisConfig::from_env()).build().await?;

    // ── Repositories ──────────────────────────────────────────────────────────

    let conversation_repo = Arc::new(ScyllaConversationRepository::new(Arc::clone(&scylla_client)));
    let message_repo = Arc::new(ScyllaMessageRepository::new(
        Arc::clone(&scylla_client),
        config.message_bucket_hours,
    ));
    let member_repo = Arc::new(ScyllaMemberRepository::new(Arc::clone(&scylla_client)));
    let subscription_repo =
        Arc::new(ScyllaSubscriptionRepository::new(Arc::clone(&scylla_client)));

    // ── Cache / routing adapters ──────────────────────────────────────────────

    let hot_tail = Arc::new(RedisHotTailCache::new(redis_client.clone()));
    let presence = Arc::new(RedisPresenceStore::new(redis_client.clone()));
    let receipt = Arc::new(RedisReceiptStore::new(redis_client.clone()));
    let routing = Arc::new(RedisRoutingRegistry::new(redis_client.clone()));
    let broadcaster = Arc::new(RedisPlaneBroadcaster::new(redis_client.clone()));

    // ── Kafka ─────────────────────────────────────────────────────────────────

    let kafka_config = KafkaClientConfig::from_env();
    let producer =
        KafkaProducerBuilder::new(ProducerConfig::new(kafka_config.clone())).build()?;
    let publisher = Arc::new(KafkaEventPublisher::new(producer));

    // ── In-process fan-out registries + subscriber ────────────────────────────

    let member_registry =
        Arc::new(ConversationBroadcastRegistry::new(config.member_stream_buffer_size));
    let audience_registry =
        Arc::new(ConversationBroadcastRegistry::new(config.audience_stream_buffer_size));

    let sink = Arc::new(PlaneFanoutSink::new(
        Arc::clone(&member_registry),
        Arc::clone(&audience_registry),
    ));
    let plane_subscriber = Arc::new(PlaneSubscriber::new(redis_subscriber, Arc::clone(&sink)));
    Arc::clone(&plane_subscriber).spawn();

    // ── Message-fork orchestrator ─────────────────────────────────────────────

    let fanout = Arc::new(MessageFanout::new(
        Arc::clone(&broadcaster),
        Arc::clone(&routing),
        Arc::clone(&hot_tail),
        config.hot_tail_cache_size,
        config.presence_ttl_secs,
    ));

    // ── CQRS buses ────────────────────────────────────────────────────────────

    let command_bus = CommandBusBuilder::new()
        .register::<CreateConversationCommand, _>(CreateConversationHandler {
            conversation_repo: Arc::clone(&conversation_repo),
            member_repo:       Arc::clone(&member_repo),
            publisher:         Arc::clone(&publisher),
        })?
        .register::<SendMessageCommand, _>(SendMessageHandler {
            member_repo:  Arc::clone(&member_repo),
            message_repo: Arc::clone(&message_repo),
            publisher:    Arc::clone(&publisher),
        })?
        .register::<ToggleVisibilityCommand, _>(ToggleVisibilityHandler {
            conversation_repo: Arc::clone(&conversation_repo),
            member_repo:       Arc::clone(&member_repo),
            publisher:         Arc::clone(&publisher),
        })?
        .register::<JoinAsMemberCommand, _>(JoinAsMemberHandler {
            conversation_repo: Arc::clone(&conversation_repo),
            member_repo:       Arc::clone(&member_repo),
            publisher:         Arc::clone(&publisher),
        })?
        .register::<SubscribeCommand, _>(SubscribeHandler {
            conversation_repo: Arc::clone(&conversation_repo),
            subscription_repo: Arc::clone(&subscription_repo),
        })?
        .register::<UnsubscribeCommand, _>(UnsubscribeHandler {
            subscription_repo: Arc::clone(&subscription_repo),
        })?
        .register::<MarkReadCommand, _>(MarkReadHandler {
            member_repo: Arc::clone(&member_repo),
        })?
        .build();

    let query_bus = QueryBusBuilder::new()
        .register::<GetHistoryQuery, _>(GetHistoryHandler {
            conversation_repo: Arc::clone(&conversation_repo),
            member_repo:       Arc::clone(&member_repo),
            message_repo:      Arc::clone(&message_repo),
            max_page_size:     config.max_page_size,
        })?
        .register::<ListMembersQuery, _>(ListMembersHandler {
            member_repo: Arc::clone(&member_repo),
        })?
        .register::<ListSubscriptionsQuery, _>(ListSubscriptionsHandler {
            subscription_repo: Arc::clone(&subscription_repo),
            max_page_size:     config.max_page_size,
        })?
        .build();

    // ── Registry reapers ──────────────────────────────────────────────────────

    tokio::spawn(Arc::clone(&member_registry).run_reaper());
    tokio::spawn(Arc::clone(&audience_registry).run_reaper());

    // ── Kafka workers ─────────────────────────────────────────────────────────

    // Every pod consumes unpublish events to tear down the Audience Plane locally.
    let visibility_worker = VisibilityWorker::new(
        kafka_config.clone(),
        Arc::clone(&audience_registry),
        Arc::clone(&routing) as Arc<dyn RoutingRegistry>,
        "chat-visibility-consumer",
    );
    tokio::spawn(visibility_worker.run());

    // ── gRPC handler ──────────────────────────────────────────────────────────

    let params = StreamingParams {
        presence_ttl_secs:    config.presence_ttl_secs,
        typing_ttl_secs:      config.typing_ttl_secs,
        audience_shard_count: config.audience_shard_count,
        audience_ttl_secs:    config.presence_ttl_secs,
    };

    let handler = ChatServiceHandler::new(
        command_bus,
        query_bus,
        Arc::clone(&fanout) as Arc<dyn Fanout>,
        Arc::clone(&plane_subscriber) as Arc<dyn PlaneAttach>,
        Arc::clone(&member_registry),
        Arc::clone(&audience_registry),
        Arc::clone(&presence) as Arc<dyn PresenceStore>,
        Arc::clone(&receipt) as Arc<dyn ReceiptStore>,
        Arc::clone(&routing) as Arc<dyn RoutingRegistry>,
        Arc::clone(&conversation_repo) as Arc<dyn ConversationRepository>,
        Arc::clone(&member_repo) as Arc<dyn MemberRepository>,
        params,
    );

    // ── Serve ─────────────────────────────────────────────────────────────────

    let (health_reporter, health_service) = health_reporter();
    health_reporter
        .set_serving::<ChatServiceServer<ChatServiceHandler<InMemoryCommandBus, InMemoryQueryBus>>>()
        .await;

    let reflection = ReflectionBuilder::configure()
        .register_encoded_file_descriptor_set(FILE_DESCRIPTOR_SET)
        .build_v1()?;

    let svc = ChatServiceServer::new(handler);

    tracing::info!(addr = %addr, "chat gRPC server listening");

    Server::builder()
        .add_service(health_service)
        .add_service(reflection)
        .add_service(svc)
        .serve(addr)
        .await?;

    Ok(())
}
