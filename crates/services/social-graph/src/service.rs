//! Adapts the social-graph composition root to the fleet
//! [`service_runtime::Service`] contract so the shared runtime can host it.
//!
//! Domain wiring stays in [`crate::app`]; this module maps env → config, builds
//! the concrete Kafka event publisher, defers to [`App::build`], registers the
//! gRPC services, and exposes the backend health probes.

use std::sync::Arc;

use async_trait::async_trait;
use cqrs::command::InMemoryCommandBus;
use cqrs::query::InMemoryQueryBus;
use redis_storage::RedisConfig;
use scylla_storage::ScyllaConfig;
use service_runtime::{HealthProbe, InfraRegistry, Service};
use tonic::service::RoutesBuilder;
use tonic_reflection::server::Builder as ReflectionBuilder;
use transport::kafka::config::{KafkaClientConfig, ProducerConfig};
use transport::kafka::producer::KafkaProducerBuilder;

use crate::domain::value_object::TierThresholds;

/// Default follower-count floors for tier classification (a product default; tune
/// via `SOCIAL_GRAPH_PREMIUM_FOLLOWER_THRESHOLD` / `..._VIP_FOLLOWER_THRESHOLD`).
const DEFAULT_PREMIUM_FOLLOWERS: i64 = 10_000;
const DEFAULT_VIP_FOLLOWERS: i64 = 1_000_000;

fn tier_thresholds_from_env() -> TierThresholds {
    TierThresholds::new(
        env_i64("SOCIAL_GRAPH_PREMIUM_FOLLOWER_THRESHOLD", DEFAULT_PREMIUM_FOLLOWERS),
        env_i64("SOCIAL_GRAPH_VIP_FOLLOWER_THRESHOLD", DEFAULT_VIP_FOLLOWERS),
    )
}

fn env_i64(key: &str, default: i64) -> i64 {
    std::env::var(key).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

use crate::app::{App, Backends};
use crate::application::port::EventPublisher;
use crate::infrastructure::grpc::handler::social_graph_service_handler::SocialGraphServiceServer;
use crate::infrastructure::grpc::handler::SocialGraphServiceHandler;
use crate::infrastructure::grpc::server::FILE_DESCRIPTOR_SET;
use crate::infrastructure::publisher::KafkaEventPublisher;

type SocialGraphServer =
    SocialGraphServiceServer<SocialGraphServiceHandler<Arc<InMemoryCommandBus>, Arc<InMemoryQueryBus>>>;

/// The social-graph service as hosted by [`service_runtime`].
pub struct SocialGraphService {
    app: App,
}

#[async_trait]
impl Service for SocialGraphService {
    const NAME: &'static str = "social-graph";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");
    const GRPC_SERVICE_NAME: &'static str =
        <SocialGraphServer as tonic::server::NamedService>::NAME;

    async fn build(_infra: Arc<InfraRegistry>) -> anyhow::Result<Self> {
        let backends = Backends {
            scylla: ScyllaConfig::from_env(),
            redis:  RedisConfig::from_env(),
        };

        // Social-graph always publishes downstream events; build the durable
        // Kafka publisher from env.
        let producer = KafkaProducerBuilder::new(ProducerConfig::new(KafkaClientConfig::from_env()))
            .build()?;
        let publisher: Arc<dyn EventPublisher> = Arc::new(KafkaEventPublisher::new(producer));

        let app = App::build(backends, publisher, tier_thresholds_from_env())
            .await
            .map_err(|e| anyhow::anyhow!("social-graph app build: {e}"))?;

        Ok(Self { app })
    }

    fn health_probes(&self) -> Vec<Arc<dyn HealthProbe>> {
        vec![
            scylla_storage::health::probe(Arc::clone(&self.app.scylla)),
            redis_storage::health::probe((*self.app.redis).clone()),
        ]
    }

    fn register(self, routes: &mut RoutesBuilder) -> anyhow::Result<()> {
        let handler = SocialGraphServiceHandler::new(
            Arc::clone(&self.app.command_bus),
            Arc::clone(&self.app.query_bus),
        );
        let reflection = ReflectionBuilder::configure()
            .register_encoded_file_descriptor_set(FILE_DESCRIPTOR_SET)
            .build_v1()?;

        routes.add_service(reflection);
        routes.add_service(SocialGraphServiceServer::new(handler));
        Ok(())
    }
}
