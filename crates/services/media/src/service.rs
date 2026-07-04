//! Adapts the media composition root to the fleet [`service_runtime::Service`]
//! contract. Maps env → config, defers to [`App::build`], self-spawns the inbound
//! consumers (the Plane B processing worker + the moderation takedown consumer),
//! registers the concrete tonic service, and reports Postgres + Redis + object-store
//! liveness.
//!
//! The processing consumer is the worker role; in production it can be scaled as a
//! separate deployment of this same image (more replicas absorbing transcode load
//! without touching the control-plane RPC latency).

use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use async_trait::async_trait;
use postgres_storage::PostgresConfig;
use redis_storage::RedisConfig;
use service_runtime::{FnProbe, HealthProbe, InfraRegistry, Service};
use tonic::service::RoutesBuilder;
use tonic_reflection::server::Builder as ReflectionBuilder;
use transport::kafka::config::{ConsumerConfig, KafkaClientConfig, ProducerConfig};
use transport::kafka::consumer::{KafkaConsumerBuilder, KafkaConsumerHandle};
use transport::kafka::producer::{KafkaProducerBuilder, KafkaProducerHandle};

use crate::app::{App, Backends};
use crate::application::command::{ApplyModerationHandler, ProcessAssetHandler};
use crate::config::MediaConfig;
use crate::infrastructure::consumer::{run_moderation_consumer, run_process_consumer};
use crate::infrastructure::grpc::{FILE_DESCRIPTOR_SET, MediaServiceHandler, MediaServiceServer};

const MEDIA_TOPIC: &str = "media.v1.events";
const PROCESS_GROUP: &str = "media-processor";
const MODERATION_TOPIC: &str = "moderation.v1.events";
const MODERATION_GROUP: &str = "media-moderation-consumer";
/// Backoff before respawning a consumer after the runner returns.
const CONSUMER_RESPAWN_BACKOFF: Duration = Duration::from_secs(5);

type MediaServer = MediaServiceServer<MediaServiceHandler>;

/// The media service as hosted by [`service_runtime`].
pub struct MediaService {
    app: App,
}

#[async_trait]
impl Service for MediaService {
    const NAME: &'static str = "media";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");
    const GRPC_SERVICE_NAME: &'static str = <MediaServer as tonic::server::NamedService>::NAME;

    async fn build(_infra: Arc<InfraRegistry>) -> anyhow::Result<Self> {
        let config = MediaConfig::from_env();
        let backends = Backends {
            postgres: PostgresConfig::from_env(),
            redis: RedisConfig::from_env(),
            kafka: Some(KafkaClientConfig::from_env()),
        };

        let app = App::build(config, backends)
            .await
            .map_err(|e| anyhow::anyhow!("media app build: {e}"))?;

        // Plane B pipeline (off media.v1.events) + moderation takedowns.
        spawn_process_consumer(Arc::clone(&app.process));
        spawn_moderation_consumer(Arc::clone(&app.apply_moderation));

        Ok(Self { app })
    }

    fn health_probes(&self) -> Vec<Arc<dyn HealthProbe>> {
        let store = Arc::clone(&self.app.store);
        vec![
            postgres_storage::health::probe(self.app.pool.clone()),
            redis_storage::health::probe(self.app.redis.clone()),
            Arc::new(FnProbe::new("object-store", move || {
                let store = Arc::clone(&store);
                async move { store.health().await.map_err(anyhow::Error::from) }
            })),
        ]
    }

    fn register(self, routes: &mut RoutesBuilder) -> anyhow::Result<()> {
        let reflection = ReflectionBuilder::configure()
            .register_encoded_file_descriptor_set(FILE_DESCRIPTOR_SET)
            .build_v1()?;

        routes.add_service(reflection);
        routes.add_service(MediaServiceServer::new(self.app.handler));
        Ok(())
    }
}

/// Spawns the supervised Plane B processing consumer.
fn spawn_process_consumer(handler: Arc<ProcessAssetHandler>) {
    tokio::spawn(async move {
        loop {
            match build_consumer(MEDIA_TOPIC, PROCESS_GROUP) {
                Ok((consumer, producer)) => {
                    run_process_consumer(consumer, Arc::clone(&handler), producer).await;
                    tracing::warn!("media processing consumer exited; respawning after backoff");
                }
                Err(error) => {
                    tracing::error!(%error, "failed to build processing consumer; retrying")
                }
            }
            tokio::time::sleep(CONSUMER_RESPAWN_BACKOFF).await;
        }
    });
}

/// Spawns the supervised moderation takedown consumer.
fn spawn_moderation_consumer(handler: Arc<ApplyModerationHandler>) {
    tokio::spawn(async move {
        loop {
            match build_consumer(MODERATION_TOPIC, MODERATION_GROUP) {
                Ok((consumer, producer)) => {
                    run_moderation_consumer(consumer, Arc::clone(&handler), producer).await;
                    tracing::warn!("media moderation consumer exited; respawning after backoff");
                }
                Err(error) => {
                    tracing::error!(%error, "failed to build moderation consumer; retrying")
                }
            }
            tokio::time::sleep(CONSUMER_RESPAWN_BACKOFF).await;
        }
    });
}

/// Builds a manual-commit consumer (subscribed to `topic`) and the dead-letter
/// producer the runner needs.
fn build_consumer(
    topic: &str,
    group: &str,
) -> anyhow::Result<(KafkaConsumerHandle, KafkaProducerHandle)> {
    let kafka = KafkaClientConfig::from_env();
    let consumer = KafkaConsumerBuilder::new(ConsumerConfig::new(kafka.clone(), group))
        .subscribe(topic)
        .build()
        .with_context(|| format!("build consumer for {topic}"))?;
    let producer = KafkaProducerBuilder::new(ProducerConfig::new(kafka))
        .build()
        .with_context(|| format!("build dead-letter producer for {topic}"))?;
    Ok((consumer, producer))
}
