//! Adapts the post composition root to the fleet [`service_runtime::Service`]
//! contract. Post is ScyllaDB-only and always publishes domain events through the
//! durable Kafka publisher.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use cqrs::command::InMemoryCommandBus;
use cqrs::query::InMemoryQueryBus;
use scylla_storage::ScyllaConfig;
use service_runtime::{HealthProbe, InfraRegistry, Service};
use tonic::service::RoutesBuilder;
use tonic_reflection::server::Builder as ReflectionBuilder;
use transport::kafka::config::{ConsumerConfig, KafkaClientConfig, ProducerConfig};
use transport::kafka::consumer::{KafkaConsumerBuilder, KafkaConsumerHandle};
use transport::kafka::producer::{KafkaProducerBuilder, KafkaProducerHandle};

use crate::app::{App, Backends};
use crate::application::port::AuthorTierStore;
use crate::infrastructure::consumer::run_author_tier_consumer;
use crate::infrastructure::grpc::handler::post_service_handler::PostServiceServer;
use crate::infrastructure::grpc::handler::PostServiceHandler;
use crate::infrastructure::grpc::server::FILE_DESCRIPTOR_SET;
use crate::infrastructure::publisher::KafkaEventPublisher;

/// The profile event stream post denormalizes author tier from.
const PROFILE_EVENTS_TOPIC: &str = "profile.v1.events";
/// Consumer group for post's author-tier projection consumer.
const AUTHOR_TIER_GROUP: &str = "post-author-tier";
/// Backoff before respawning the consumer after the runner returns.
const CONSUMER_RESPAWN_BACKOFF: Duration = Duration::from_secs(5);

type PostServer =
    PostServiceServer<PostServiceHandler<Arc<InMemoryCommandBus>, Arc<InMemoryQueryBus>>>;

/// The post service as hosted by [`service_runtime`].
pub struct PostService {
    app: App,
}

#[async_trait]
impl Service for PostService {
    const NAME: &'static str = "post";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");
    const GRPC_SERVICE_NAME: &'static str = <PostServer as tonic::server::NamedService>::NAME;

    async fn build(_infra: Arc<InfraRegistry>) -> anyhow::Result<Self> {
        let backends = Backends {
            scylla: ScyllaConfig::from_env(),
        };

        let producer = KafkaProducerBuilder::new(ProducerConfig::new(KafkaClientConfig::from_env()))
            .build()?;
        let publisher = Arc::new(KafkaEventPublisher::new(producer));

        let app = App::build(backends, publisher)
            .await
            .map_err(|e| anyhow::anyhow!("post app build: {e}"))?;

        // Inbound integration: profile tier signal → denormalized author-tier
        // projection (read on the publish path to stamp posts).
        spawn_author_tier_consumer(Arc::clone(&app.author_tier_store));

        Ok(Self { app })
    }

    fn health_probes(&self) -> Vec<Arc<dyn HealthProbe>> {
        vec![scylla_storage::health::probe(Arc::clone(&self.app.scylla))]
    }

    fn register(self, routes: &mut RoutesBuilder) -> anyhow::Result<()> {
        let handler = PostServiceHandler::new(
            Arc::clone(&self.app.command_bus),
            Arc::clone(&self.app.query_bus),
        );
        let reflection = ReflectionBuilder::configure()
            .register_encoded_file_descriptor_set(FILE_DESCRIPTOR_SET)
            .build_v1()?;

        routes.add_service(reflection);
        routes.add_service(PostServiceServer::new(handler));
        Ok(())
    }
}

/// Spawns the supervised author-tier projection consumer (profile.v1.events →
/// `post.author_tiers`), respawning after a backoff whenever the runner returns.
fn spawn_author_tier_consumer(store: Arc<dyn AuthorTierStore>) {
    tokio::spawn(async move {
        loop {
            match build_author_tier_consumer() {
                Ok((consumer, producer)) => {
                    run_author_tier_consumer(consumer, Arc::clone(&store), producer).await;
                    tracing::warn!("author-tier consumer exited; respawning after backoff");
                }
                Err(error) => {
                    tracing::error!(%error, "failed to build author-tier consumer; retrying");
                }
            }
            tokio::time::sleep(CONSUMER_RESPAWN_BACKOFF).await;
        }
    });
}

/// Builds the manual-commit consumer (subscribed to `profile.v1.events`) and the
/// dead-letter producer the runner needs.
fn build_author_tier_consumer() -> anyhow::Result<(KafkaConsumerHandle, KafkaProducerHandle)> {
    let kafka = KafkaClientConfig::from_env();
    let consumer = KafkaConsumerBuilder::new(ConsumerConfig::new(kafka.clone(), AUTHOR_TIER_GROUP))
        .subscribe(PROFILE_EVENTS_TOPIC)
        .build()
        .map_err(|e| anyhow::anyhow!("build author-tier consumer: {e}"))?;
    let producer = KafkaProducerBuilder::new(ProducerConfig::new(kafka))
        .build()
        .map_err(|e| anyhow::anyhow!("build author-tier dead-letter producer: {e}"))?;
    Ok((consumer, producer))
}
