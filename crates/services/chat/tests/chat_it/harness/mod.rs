//! Integration harness: boots the infra, wires a real service graph against it,
//! and exposes the handler plus assertion back-doors.
//!
//! The graph is assembled exactly like [`chat::infrastructure::grpc::server`] but
//! with container-backed configs and per-scenario knobs ([`HarnessOptions`]),
//! and it holds the *same* `Arc`s the handler holds — so a test reads the live
//! state the handler mutates (presence sets, the shard registry, the in-process
//! broadcast registries), never a parallel copy.
//!
//! Tests drive the service through the generated [`ChatService`] gRPC trait
//! in-process (the crate builds no client; `build_client(false)`), which also
//! gives deterministic control over a stream's lifetime — essential for the RAII
//! and backpressure scenarios.
#![allow(dead_code)]

mod infra;
mod migrate;

use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use tokio::sync::broadcast;
use uuid::Uuid;

use cqrs::command::{CommandBusBuilder, InMemoryCommandBus};
use cqrs::query::{InMemoryQueryBus, QueryBusBuilder};
use redis_storage::{RedisClientBuilder, RedisConfig, RedisSubscriberBuilder};
use scylla_storage::{ScyllaConfig, ScyllaSessionBuilder};
use transport::kafka::config::client::KafkaClientConfig;
use transport::kafka::config::producer::ProducerConfig;
use transport::kafka::producer::KafkaProducerBuilder;

use chat::application::command::{
    CreateConversationCommand, CreateConversationHandler, JoinAsMemberCommand, JoinAsMemberHandler,
    MarkReadCommand, MarkReadHandler, SendMessageCommand, SendMessageHandler, SubscribeCommand,
    SubscribeHandler, ToggleVisibilityCommand, ToggleVisibilityHandler, UnsubscribeCommand,
    UnsubscribeHandler,
};
use chat::application::port::{
    ConversationRepository, EventPublisher, HotTailCache, MemberRepository, PresenceStore,
    ReceiptStore, RoutingRegistry,
};
use chat::application::query::{
    GetHistoryHandler, GetHistoryQuery, ListMembersHandler, ListMembersQuery,
    ListSubscriptionsHandler, ListSubscriptionsQuery,
};
use chat::infrastructure::cache::{
    RedisHotTailCache, RedisPresenceStore, RedisReceiptStore, RedisRoutingRegistry,
};
use chat::infrastructure::event::{KafkaEventPublisher, LogEventPublisher};
use chat::infrastructure::grpc::handler::ChatServiceHandler;
use chat::infrastructure::persistence::{
    ScyllaConversationRepository, ScyllaMemberRepository, ScyllaMessageRepository,
    ScyllaSubscriptionRepository,
};
use chat::infrastructure::routing::{
    Fanout, MessageFanout, PlaneAttach, PlaneSubscriber, RedisPlaneBroadcaster,
};
use chat::infrastructure::streaming::{ConversationBroadcastRegistry, PlaneFanoutSink};
use chat::infrastructure::worker::VisibilityWorker;

// ── Re-exports the scenarios drive the service through ───────────────────────

pub use chat::domain::value_object::{ConversationId, MessageId, ProfileId};
pub use chat::infrastructure::grpc::handler::chat_handler::proto;
pub use chat::infrastructure::grpc::handler::chat_handler::proto::chat_service_server::ChatService;
pub use chat::infrastructure::grpc::handler::chat_handler::StreamingParams;
pub use chat::infrastructure::routing::PlaneEvent;
pub use tonic::{Request, Status};

/// Generous default patience for a cross-component assertion (Redis pub/sub fan-out
/// round-trip, async Drop cleanup, etc.).
pub const DEADLINE: Duration = Duration::from_secs(10);
/// Patience for assertions that span a Kafka consumer-group join + consume.
pub const KAFKA_DEADLINE: Duration = Duration::from_secs(30);

/// ScyllaDB message-log bucket size (hours). Mirrors the production default.
const BUCKET_HOURS: u32 = 24;

/// Per-scenario knobs. Defaults mirror production; scenarios shrink buffers/TTLs
/// to make overflow and liveness assertions complete in seconds.
#[derive(Debug, Clone)]
pub struct HarnessOptions {
    pub member_buffer:        usize,
    pub audience_buffer:      usize,
    pub audience_shard_count: u16,
    pub presence_ttl_secs:    u64,
    pub typing_ttl_secs:      u64,
    pub audience_ttl_secs:    u64,
    pub hot_tail_cap:         u16,
    pub max_page_size:        i32,
    /// Boot Kafka, publish via [`KafkaEventPublisher`], and run the
    /// [`VisibilityWorker`]. Off by default so scenarios 1–3 never boot Kafka.
    pub with_kafka:           bool,
}

impl Default for HarnessOptions {
    fn default() -> Self {
        Self {
            member_buffer:        256,
            audience_buffer:      1024,
            // One shard keeps every guest on shard 0, so audience refcount/privacy
            // assertions are deterministic without hunting for hash collisions.
            audience_shard_count: 1,
            presence_ttl_secs:    30,
            typing_ttl_secs:      6,
            audience_ttl_secs:    30,
            hot_tail_cap:         200,
            max_page_size:        100,
            with_kafka:           false,
        }
    }
}

/// A fully-wired chat service bound to ephemeral infra, plus assertion handles.
pub struct TestHarness {
    pub handler:           ChatServiceHandler<InMemoryCommandBus, InMemoryQueryBus>,
    pub presence:          Arc<dyn PresenceStore>,
    pub routing:           Arc<dyn RoutingRegistry>,
    pub hot_tail:          Arc<dyn HotTailCache>,
    pub member_registry:   Arc<ConversationBroadcastRegistry>,
    pub audience_registry: Arc<ConversationBroadcastRegistry>,
    pub params:            StreamingParams,
}

impl TestHarness {
    /// Boots/reuses the shared containers, applies migrations, and assembles the
    /// service graph for these options.
    pub async fn start(opts: HarnessOptions) -> Self {
        // ── Storage clients (fresh per harness; the containers are shared) ──────
        let scylla_cp = infra::scylla_contact_point().await;
        let scylla = Arc::new(
            ScyllaSessionBuilder::new(ScyllaConfig {
                contact_points: vec![scylla_cp],
                keyspace:       None,
                ..ScyllaConfig::default()
            })
            .build()
            .await
            .expect("integration: ScyllaDB session"),
        );

        let redis_endpoint = infra::redis_endpoint().await;
        let redis_config = RedisConfig { hosts: vec![redis_endpoint], ..RedisConfig::default() };
        let redis_client = RedisClientBuilder::new(redis_config.clone())
            .build()
            .await
            .expect("integration: Redis client");
        let redis_subscriber = RedisSubscriberBuilder::new(redis_config)
            .build()
            .await
            .expect("integration: Redis subscriber");

        // ── Kafka (only when the scenario asks) ─────────────────────────────────
        let kafka_config = if opts.with_kafka {
            let brokers = infra::kafka_brokers().await;
            infra::ensure_topics(
                &brokers,
                &[
                    "chat.conversation.created",
                    "chat.conversation.published",
                    "chat.conversation.unpublished",
                    "chat.conversation.unpublished.dlq",
                    "chat.member.joined",
                    "chat.member.left",
                    "chat.message.sent",
                ],
            )
            .await;
            Some(KafkaClientConfig::new(brokers))
        } else {
            None
        };

        // ── Repositories ────────────────────────────────────────────────────────
        let conversation_repo = Arc::new(ScyllaConversationRepository::new(Arc::clone(&scylla)));
        let message_repo = Arc::new(ScyllaMessageRepository::new(Arc::clone(&scylla), BUCKET_HOURS));
        let member_repo = Arc::new(ScyllaMemberRepository::new(Arc::clone(&scylla)));
        let subscription_repo = Arc::new(ScyllaSubscriptionRepository::new(Arc::clone(&scylla)));

        // ── Cache / routing adapters ────────────────────────────────────────────
        let hot_tail = Arc::new(RedisHotTailCache::new(redis_client.clone()));
        let presence = Arc::new(RedisPresenceStore::new(redis_client.clone()));
        let receipt = Arc::new(RedisReceiptStore::new(redis_client.clone()));
        let routing = Arc::new(RedisRoutingRegistry::new(redis_client.clone()));
        let broadcaster = Arc::new(RedisPlaneBroadcaster::new(redis_client.clone()));

        // ── In-process fan-out registries + per-pod subscriber ──────────────────
        let member_registry = Arc::new(ConversationBroadcastRegistry::new(opts.member_buffer));
        let audience_registry = Arc::new(ConversationBroadcastRegistry::new(opts.audience_buffer));
        let sink = Arc::new(PlaneFanoutSink::new(
            Arc::clone(&member_registry),
            Arc::clone(&audience_registry),
        ));
        let plane_subscriber = Arc::new(PlaneSubscriber::new(redis_subscriber, sink));
        Arc::clone(&plane_subscriber).spawn();

        // ── Message-fork orchestrator ───────────────────────────────────────────
        let fanout = Arc::new(MessageFanout::new(
            Arc::clone(&broadcaster),
            Arc::clone(&routing),
            Arc::clone(&hot_tail),
            opts.hot_tail_cap,
            opts.audience_ttl_secs,
        ));

        // ── CQRS buses (publisher chosen per scenario) ──────────────────────────
        let command_bus = match &kafka_config {
            Some(cfg) => {
                let producer = KafkaProducerBuilder::new(ProducerConfig::new(cfg.clone()))
                    .build()
                    .expect("integration: Kafka producer");
                build_command_bus(
                    Arc::new(KafkaEventPublisher::new(producer)),
                    &conversation_repo,
                    &message_repo,
                    &member_repo,
                    &subscription_repo,
                )
            }
            None => build_command_bus(
                Arc::new(LogEventPublisher),
                &conversation_repo,
                &message_repo,
                &member_repo,
                &subscription_repo,
            ),
        };

        let query_bus = QueryBusBuilder::new()
            .register::<GetHistoryQuery, _>(GetHistoryHandler {
                conversation_repo: Arc::clone(&conversation_repo),
                member_repo:       Arc::clone(&member_repo),
                message_repo:      Arc::clone(&message_repo),
                max_page_size:     opts.max_page_size,
            })
            .expect("register GetHistory")
            .register::<ListMembersQuery, _>(ListMembersHandler {
                member_repo: Arc::clone(&member_repo),
            })
            .expect("register ListMembers")
            .register::<ListSubscriptionsQuery, _>(ListSubscriptionsHandler {
                subscription_repo: Arc::clone(&subscription_repo),
                max_page_size:     opts.max_page_size,
            })
            .expect("register ListSubscriptions")
            .build();

        // ── VisibilityWorker (Kafka path): cluster-wide Audience-Plane teardown ──
        if let Some(cfg) = &kafka_config {
            let worker = VisibilityWorker::new(
                cfg.clone(),
                Arc::clone(&audience_registry),
                Arc::clone(&routing) as Arc<dyn RoutingRegistry>,
                format!("chat-it-visibility-{}", Uuid::now_v7()),
            );
            tokio::spawn(worker.run());
        }

        // ── Registry reapers (production parity; 60 s cadence — tests reap by hand)
        tokio::spawn(Arc::clone(&member_registry).run_reaper());
        tokio::spawn(Arc::clone(&audience_registry).run_reaper());

        let params = StreamingParams {
            presence_ttl_secs:    opts.presence_ttl_secs,
            typing_ttl_secs:      opts.typing_ttl_secs,
            audience_shard_count: opts.audience_shard_count,
            audience_ttl_secs:    opts.audience_ttl_secs,
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

        Self {
            handler,
            presence: presence as Arc<dyn PresenceStore>,
            routing: routing as Arc<dyn RoutingRegistry>,
            hot_tail: hot_tail as Arc<dyn HotTailCache>,
            member_registry,
            audience_registry,
            params,
        }
    }

    // ── Driving helpers (through the gRPC trait) ────────────────────────────────

    /// Creates a `Channel` (born `Public`); the owner is its sole roster member.
    pub async fn create_public_channel(&self, owner: &ProfileId) -> ConversationId {
        let resp = ChatService::create_conversation(
            &self.handler,
            Request::new(proto::CreateConversationRequest { kind: 1, owner_id: owner.as_str() }),
        )
        .await
        .expect("create_conversation");
        ConversationId::try_from(resp.into_inner().conversation_id.as_str())
            .expect("valid conversation id")
    }

    /// Sends a text message and returns its server-minted id.
    pub async fn send_text(&self, conv: &ConversationId, sender: &ProfileId, body: &str) -> MessageId {
        let resp = ChatService::send_message(
            &self.handler,
            Request::new(proto::SendMessageRequest {
                conversation_id: conv.as_str(),
                sender_id:       sender.as_str(),
                content_type:    0, // Text
                body:            body.to_owned(),
                media_ref:       String::new(),
                reply_to:        String::new(),
            }),
        )
        .await
        .expect("send_message");
        MessageId::try_from(resp.into_inner().message_id.as_str()).expect("valid message id")
    }

    /// Opens a Member-Plane stream and returns the raw response stream.
    pub async fn open_member_stream(
        &self,
        conv:   &ConversationId,
        member: &ProfileId,
    ) -> ResponseStream<proto::StreamConversationResponse> {
        ChatService::stream_conversation(
            &self.handler,
            Request::new(proto::StreamConversationRequest {
                conversation_id: conv.as_str(),
                member_id:       member.as_str(),
            }),
        )
        .await
        .expect("stream_conversation")
        .into_inner()
    }

    /// Opens an Audience-Plane stream and returns the raw response stream.
    pub async fn open_public_stream(
        &self,
        conv:       &ConversationId,
        subscriber: &ProfileId,
    ) -> ResponseStream<proto::StreamPublicResponse> {
        ChatService::stream_public(
            &self.handler,
            Request::new(proto::StreamPublicRequest {
                conversation_id: conv.as_str(),
                subscriber_id:   subscriber.as_str(),
            }),
        )
        .await
        .expect("stream_public")
        .into_inner()
    }
}

/// Concrete shape of the handler's boxed server-streaming response.
pub type ResponseStream<T> =
    std::pin::Pin<Box<dyn futures::Stream<Item = Result<T, Status>> + Send + 'static>>;

/// Builds the command bus generically over the event publisher so the same wiring
/// serves both the log-backed (scenarios 1–3) and Kafka-backed (scenario 4) runs.
fn build_command_bus<EP: EventPublisher>(
    publisher:         Arc<EP>,
    conversation_repo: &Arc<ScyllaConversationRepository>,
    message_repo:      &Arc<ScyllaMessageRepository>,
    member_repo:       &Arc<ScyllaMemberRepository>,
    subscription_repo: &Arc<ScyllaSubscriptionRepository>,
) -> InMemoryCommandBus {
    CommandBusBuilder::new()
        .register::<CreateConversationCommand, _>(CreateConversationHandler {
            conversation_repo: Arc::clone(conversation_repo),
            member_repo:       Arc::clone(member_repo),
            publisher:         Arc::clone(&publisher),
        })
        .expect("register CreateConversation")
        .register::<SendMessageCommand, _>(SendMessageHandler {
            member_repo:  Arc::clone(member_repo),
            message_repo: Arc::clone(message_repo),
            publisher:    Arc::clone(&publisher),
        })
        .expect("register SendMessage")
        .register::<ToggleVisibilityCommand, _>(ToggleVisibilityHandler {
            conversation_repo: Arc::clone(conversation_repo),
            member_repo:       Arc::clone(member_repo),
            publisher:         Arc::clone(&publisher),
        })
        .expect("register ToggleVisibility")
        .register::<JoinAsMemberCommand, _>(JoinAsMemberHandler {
            conversation_repo: Arc::clone(conversation_repo),
            member_repo:       Arc::clone(member_repo),
            publisher:         Arc::clone(&publisher),
        })
        .expect("register JoinAsMember")
        .register::<SubscribeCommand, _>(SubscribeHandler {
            conversation_repo: Arc::clone(conversation_repo),
            subscription_repo: Arc::clone(subscription_repo),
        })
        .expect("register Subscribe")
        .register::<UnsubscribeCommand, _>(UnsubscribeHandler {
            subscription_repo: Arc::clone(subscription_repo),
        })
        .expect("register Unsubscribe")
        .register::<MarkReadCommand, _>(MarkReadHandler { member_repo: Arc::clone(member_repo) })
        .expect("register MarkRead")
        .build()
}

// ── Test utilities ───────────────────────────────────────────────────────────

/// A fresh random profile id.
pub fn random_profile() -> ProfileId {
    ProfileId::from_uuid(Uuid::now_v7())
}

/// A fresh random message id (string form), for signal RPCs that take one.
pub fn random_message_id() -> String {
    Uuid::now_v7().to_string()
}

/// Current epoch-ms, as the service stamps liveness scores.
pub fn now_ms() -> i64 {
    Utc::now().timestamp_millis()
}

/// Polls `probe` every 50 ms until it returns `true`, or panics at `deadline`.
/// The single anti-flake primitive: assertions wait on observable state, never on
/// a fixed sleep.
pub async fn await_until<F, Fut>(label: &str, deadline: Duration, mut probe: F)
where
    F:   FnMut() -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    let start = std::time::Instant::now();
    loop {
        if probe().await {
            return;
        }
        if start.elapsed() > deadline {
            panic!("await_until timed out after {deadline:?}: {label}");
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// Receives the next item from a response stream within `deadline`, or `None` on
/// timeout / stream end.
pub async fn recv<T>(stream: &mut ResponseStream<T>, deadline: Duration) -> Option<Result<T, Status>> {
    tokio::time::timeout(deadline, futures::StreamExt::next(stream))
        .await
        .ok()
        .flatten()
}

/// Receives the next in-process plane event from a raw registry tap within
/// `deadline`, or `None` on timeout.
pub async fn recv_event(
    rx:       &mut broadcast::Receiver<Arc<PlaneEvent>>,
    deadline: Duration,
) -> Option<Arc<PlaneEvent>> {
    match tokio::time::timeout(deadline, rx.recv()).await {
        Ok(Ok(event)) => Some(event),
        _ => None,
    }
}
