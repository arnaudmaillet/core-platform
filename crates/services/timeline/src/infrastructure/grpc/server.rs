use std::net::SocketAddr;
use std::sync::Arc;

use cqrs::query::InMemoryQueryBus;
use tonic::transport::{Channel, Server};
use tonic_health::server::health_reporter;
use tonic_reflection::server::Builder as ReflectionBuilder;

use redis_storage::RedisConfig;
use scylla_storage::ScyllaConfig;
use transport::kafka::config::client::KafkaClientConfig;

use crate::app::{App, AppConfig, Backends};
use crate::config::TimelineConfig;
use crate::infrastructure::client::SocialGraphGrpcClient;
use crate::infrastructure::grpc::handler::{TimelineServiceHandler, TimelineServiceServer};

/// Proto file descriptor blob embedded at build time for server reflection.
pub const FILE_DESCRIPTOR_SET: &[u8] =
    tonic::include_file_descriptor_set!("timeline_descriptor");

/// The gRPC handler type the server serves: the query bus is shared by `Arc`
/// (the same instance the composition root retains).
type ServingHandler = TimelineServiceHandler<Arc<InMemoryQueryBus>>;

/// Bootstraps and runs the timeline gRPC server with all background workers.
///
/// Reads configuration from the environment, connects the social-graph gRPC
/// client, builds the full service graph via the shared composition root
/// ([`App::build`]) — which also spawns the four ingestion workers — then binds
/// the socket and serves until shutdown.
pub async fn serve(addr: SocketAddr) -> Result<(), Box<dyn std::error::Error>> {
    let config = TimelineConfig::from_env();

    // ── Social-graph gRPC client ────────────────────────────────────────────────
    let sg_channel = Channel::from_shared(config.social_graph_endpoint.clone())?
        .connect()
        .await?;
    let social_graph = Arc::new(SocialGraphGrpcClient::new(sg_channel));

    let app_config = AppConfig {
        feed_cap:                   config.feed_cap,
        audio_feed_cap:             config.audio_feed_cap,
        vip_registry_cap:           config.vip_registry_cap,
        backfill_limit:             config.backfill_limit,
        warm_ttl_secs:              config.warm_ttl_secs,
        tier_cache_ttl_secs:        config.tier_cache_ttl_secs,
        vip_registry_ttl_secs:      config.vip_registry_ttl_secs,
        max_page_size:              config.max_page_size,
        max_vip_merge_sources:      config.max_vip_merge_sources,
        warm_max_concurrency:       config.warm_max_concurrency,
        social_graph_page_size:     config.social_graph_page_size,
        kafka_group_post_published: config.kafka_group_post_published.clone(),
        kafka_group_post_deleted:   config.kafka_group_post_deleted.clone(),
        kafka_group_sg_followed:    config.kafka_group_sg_followed.clone(),
        kafka_group_sg_unfollowed:  config.kafka_group_sg_unfollowed.clone(),
    };

    let backends = Backends {
        scylla: ScyllaConfig::from_env(),
        redis:  RedisConfig::from_env(),
        kafka:  Some(KafkaClientConfig::from_env()),
    };

    let app = App::build(&app_config, backends, social_graph).await?;

    // ── gRPC server ───────────────────────────────────────────────────────────

    let (health_reporter, health_service) = health_reporter();
    health_reporter
        .set_serving::<TimelineServiceServer<ServingHandler>>()
        .await;

    let reflection = ReflectionBuilder::configure()
        .register_encoded_file_descriptor_set(FILE_DESCRIPTOR_SET)
        .build_v1()?;

    let svc = TimelineServiceServer::new(TimelineServiceHandler::new(Arc::clone(&app.query_bus)));

    tracing::info!(addr = %addr, "timeline gRPC server listening");

    Server::builder()
        .add_service(health_service)
        .add_service(reflection)
        .add_service(svc)
        .serve(addr)
        .await?;

    Ok(())
}
