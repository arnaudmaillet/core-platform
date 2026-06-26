//! Adapts the audit composition roots to the fleet [`service_runtime::Service`]
//! contract — **twice**, because audit is two deployables that share a domain but
//! no process or failure domain:
//!
//! * [`AuditServerService`] (`audit-server`) — the read/record plane. Builds the
//!   adapters, composes the gRPC handler, serves `audit.v1` on :50068 (the
//!   synchronous fail-closed `RecordPrivileged` + the access-controlled
//!   `Query`/`Export`/`VerifyIntegrity` reads), and reports ledger liveness.
//! * [`AuditWorkerService`] (`audit-worker`) — the ingest/verify plane. Builds the
//!   adapters, spawns the supervised `audit.v1.events` ingest consumer and the
//!   checkpoint-anchor loop, and serves only health + reflection on :50069.
//!
//! Deferred worker loops (documented, not gaps): the crypto-shred consumer (needs
//! an erasure-request source) and the retention-expiry sweep (needs resolved
//! retention policies) — see the README.

use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use async_trait::async_trait;
use service_runtime::{HealthProbe, InfraRegistry, Service};
use sqlx::PgPool;
use tonic::service::RoutesBuilder;
use tonic_reflection::server::Builder as ReflectionBuilder;
use transport::kafka::config::{ConsumerConfig, KafkaClientConfig, ProducerConfig};
use transport::kafka::consumer::{KafkaConsumerBuilder, KafkaConsumerHandle};
use transport::kafka::producer::{KafkaProducerBuilder, KafkaProducerHandle};

use crate::app::{Adapters, compose_server};
use crate::application::IngestHandler;
use crate::config::AuditConfig;
use crate::infrastructure::grpc::{AuditServiceHandler, AuditServiceServer, FILE_DESCRIPTOR_SET};
use crate::infrastructure::run_audit_ingest_consumer;
use crate::infrastructure::run_checkpoint_loop;

const INGEST_TOPIC: &str = "audit.v1.events";
const INGEST_GROUP: &str = "audit-ingest";

/// Backoff before respawning the ingest consumer after its runner returns.
const CONSUMER_RESPAWN_BACKOFF: Duration = Duration::from_secs(5);

type AuditServer = AuditServiceServer<AuditServiceHandler>;

// ── Read / record server ──────────────────────────────────────────────────────

pub struct AuditServerService {
    handler: AuditServiceHandler,
    pool: PgPool,
}

#[async_trait]
impl Service for AuditServerService {
    const NAME: &'static str = "audit-server";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");
    const GRPC_SERVICE_NAME: &'static str = <AuditServer as tonic::server::NamedService>::NAME;

    async fn build(_infra: Arc<InfraRegistry>) -> anyhow::Result<Self> {
        let config = AuditConfig::from_env();
        let adapters = Adapters::build(&config).await.context("build audit adapters")?;
        let handler = compose_server(
            Arc::clone(&adapters.ledger),
            Arc::clone(&adapters.archive),
            Arc::clone(&adapters.anchor),
            Arc::clone(&adapters.clock),
            config.record_timeout,
        );
        Ok(Self {
            handler,
            pool: adapters.pool,
        })
    }

    fn health_probes(&self) -> Vec<Arc<dyn HealthProbe>> {
        vec![postgres_storage::health::probe(self.pool.clone())]
    }

    fn register(self, routes: &mut RoutesBuilder) -> anyhow::Result<()> {
        let reflection = ReflectionBuilder::configure()
            .register_encoded_file_descriptor_set(FILE_DESCRIPTOR_SET)
            .build_v1()?;
        routes.add_service(reflection);
        routes.add_service(AuditServiceServer::new(self.handler));
        Ok(())
    }
}

// ── Ingest / verify worker ────────────────────────────────────────────────────

pub struct AuditWorkerService {
    pool: PgPool,
}

#[async_trait]
impl Service for AuditWorkerService {
    const NAME: &'static str = "audit-worker";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");
    const GRPC_SERVICE_NAME: &'static str = <AuditServer as tonic::server::NamedService>::NAME;

    async fn build(_infra: Arc<InfraRegistry>) -> anyhow::Result<Self> {
        let config = AuditConfig::from_env();
        let adapters = Adapters::build(&config).await.context("build audit adapters")?;

        // The supervised async ingest lane: decode → chain → archive.
        spawn_ingest(adapters.ingest_handler());

        // The periodic checkpoint-anchor loop.
        tokio::spawn(run_checkpoint_loop(
            adapters.checkpoint_handler(),
            config.checkpoint_interval,
        ));

        Ok(Self { pool: adapters.pool })
    }

    fn health_probes(&self) -> Vec<Arc<dyn HealthProbe>> {
        vec![postgres_storage::health::probe(self.pool.clone())]
    }

    fn register(self, routes: &mut RoutesBuilder) -> anyhow::Result<()> {
        let reflection = ReflectionBuilder::configure()
            .register_encoded_file_descriptor_set(FILE_DESCRIPTOR_SET)
            .build_v1()?;
        routes.add_service(reflection);
        Ok(())
    }
}

// ── Worker wiring helpers ─────────────────────────────────────────────────────

/// Spawn the supervised ingest consumer: rebuild its Kafka handles and restart
/// after a backoff whenever the runner returns.
fn spawn_ingest(handler: Arc<IngestHandler>) {
    tokio::spawn(async move {
        loop {
            match build_consumer(INGEST_TOPIC, INGEST_GROUP) {
                Ok((consumer, producer)) => {
                    run_audit_ingest_consumer(consumer, producer, Arc::clone(&handler)).await;
                    tracing::warn!("audit ingest consumer exited; respawning after backoff");
                }
                Err(error) => {
                    tracing::error!(%error, "failed to build audit ingest consumer; retrying")
                }
            }
            tokio::time::sleep(CONSUMER_RESPAWN_BACKOFF).await;
        }
    });
}

/// Build a manual-commit consumer (subscribed to `topic`) and the dead-letter
/// producer the runner needs. Kafka config is read fresh per respawn.
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
