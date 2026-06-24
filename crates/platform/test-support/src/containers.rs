//! Lazy, shared ephemeral backends for a test binary.
//!
//! Every backend is booted through a process-wide [`OnceCell`] and shared by all
//! scenarios linked into the same test binary. Because each service compiles its
//! own test binary, "process-wide" is effectively "per service" — exactly the
//! one-container-set-per-service property the standard requires.
//!
//! ## Redis image override
//!
//! The `testcontainers-modules` redis module defaults to `redis:5.0`, which
//! predates sharded pub/sub (`SSUBSCRIBE`/`SPUBLISH`/`SUNSUBSCRIBE`, Redis 7.0).
//! Several services depend on those, so we pin a 7.x image explicitly for all.

use std::time::Duration;

use rdkafka::admin::{AdminClient, AdminOptions, NewTopic, TopicReplication};
use rdkafka::client::DefaultClientContext;
use rdkafka::config::ClientConfig;
use testcontainers::core::WaitFor;
use testcontainers::runners::AsyncRunner;
use testcontainers::{ContainerAsync, GenericImage, ImageExt};
use testcontainers_modules::kafka::apache::{Kafka, KAFKA_PORT};
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::scylladb::ScyllaDB;
use tokio::sync::OnceCell;

use crate::migrate;

/// Internal Redis port; the 7.x image exposes it and testcontainers maps it.
const REDIS_PORT: u16 = 6379;
/// Internal ScyllaDB CQL port.
const SCYLLA_CQL_PORT: u16 = 9042;
/// Internal PostgreSQL port.
const POSTGRES_PORT: u16 = 5432;

static SCYLLA: OnceCell<ContainerAsync<ScyllaDB>> = OnceCell::const_new();
static REDIS: OnceCell<ContainerAsync<GenericImage>> = OnceCell::const_new();
static KAFKA: OnceCell<ContainerAsync<Kafka>> = OnceCell::const_new();
static POSTGRES: OnceCell<ContainerAsync<Postgres>> = OnceCell::const_new();

static SCYLLA_MIGRATED: OnceCell<()> = OnceCell::const_new();
static POSTGRES_MIGRATED: OnceCell<()> = OnceCell::const_new();

// ── ScyllaDB ─────────────────────────────────────────────────────────────────

/// Boots ScyllaDB (once) and returns its `host:port` contact point.
///
/// `--developer-mode 1 --smp 1` makes a single-node boot fast and reliable on
/// untuned hosts (CI / macOS).
pub async fn scylla_contact_point() -> String {
    let container = SCYLLA
        .get_or_init(|| async {
            ScyllaDB::default()
                .with_cmd(["--developer-mode", "1", "--smp", "1"])
                .start()
                .await
                .expect("failed to start the ScyllaDB test container")
        })
        .await;

    let port = container
        .get_host_port_ipv4(SCYLLA_CQL_PORT)
        .await
        .expect("failed to resolve the mapped ScyllaDB port");
    format!("127.0.0.1:{port}")
}

/// Boots ScyllaDB (once), applies the service's `.cql` migrations from
/// `migrations_dir` (once, with the single-node `SimpleStrategy RF=1` rewrite of
/// the `keyspace`), and returns the contact point.
///
/// `keyspace` is the name the service's `0001_create_keyspace.cql` provisions;
/// `migrations_dir` is typically `concat!(env!("CARGO_MANIFEST_DIR"), "/migrations")`
/// so the suite exercises exactly the DDL that ships.
pub async fn scylla_ready(keyspace: &str, migrations_dir: &str) -> String {
    let contact_point = scylla_contact_point().await;

    SCYLLA_MIGRATED
        .get_or_init(|| {
            let cp = contact_point.clone();
            let keyspace = keyspace.to_owned();
            let dir = migrations_dir.to_owned();
            async move { migrate::scylla_apply(&cp, &keyspace, &dir).await }
        })
        .await;

    contact_point
}

// ── Redis ────────────────────────────────────────────────────────────────────

/// Boots a Redis 7.x node (once) and returns its `host:port`.
pub async fn redis_endpoint() -> String {
    let container = REDIS
        .get_or_init(|| async {
            GenericImage::new("redis", "7-alpine")
                .with_wait_for(WaitFor::message_on_stdout("Ready to accept connections"))
                .start()
                .await
                .expect("failed to start the Redis test container")
        })
        .await;

    let port = container
        .get_host_port_ipv4(REDIS_PORT)
        .await
        .expect("failed to resolve the mapped Redis port");
    format!("127.0.0.1:{port}")
}

// ── Kafka ────────────────────────────────────────────────────────────────────

/// Boots the Kafka broker (once) and returns its bootstrap `host:port`.
///
/// The `apache/kafka-native` image advertises `127.0.0.1:<mapped port>`, so
/// clients must dial that exact host.
pub async fn kafka_brokers() -> String {
    let container = KAFKA
        .get_or_init(|| async {
            Kafka::default()
                .start()
                .await
                .expect("failed to start the Kafka test container")
        })
        .await;

    let port = container
        .get_host_port_ipv4(KAFKA_PORT)
        .await
        .expect("failed to resolve the mapped Kafka port");
    format!("127.0.0.1:{port}")
}

/// Synchronously creates each topic (partitions=1, RF=1) and waits for the broker
/// to confirm, so a freshly built consumer/producer never races auto-creation.
/// A topic left over from an earlier scenario is treated as success.
pub async fn ensure_topics(brokers: &str, topics: &[&str]) {
    let admin: AdminClient<DefaultClientContext> = ClientConfig::new()
        .set("bootstrap.servers", brokers)
        .create()
        .expect("failed to build the Kafka AdminClient");

    let new_topics: Vec<NewTopic> = topics
        .iter()
        .map(|name| NewTopic::new(name, 1, TopicReplication::Fixed(1)))
        .collect();

    let opts = AdminOptions::new().operation_timeout(Some(Duration::from_secs(10)));
    let results = admin
        .create_topics(&new_topics, &opts)
        .await
        .expect("the create_topics request failed");

    for result in results {
        if let Err((topic, code)) = result
            && code != rdkafka::types::RDKafkaErrorCode::TopicAlreadyExists
        {
            panic!("failed to create topic '{topic}': {code}");
        }
    }
}

// ── Postgres ─────────────────────────────────────────────────────────────────

/// Boots PostgreSQL (once), applies the service's `.sql` migrations from
/// `migrations_dir` (once), and returns a connection URL.
///
/// Unlike ScyllaDB there is no replication rewrite — a single Postgres node is a
/// faithful production analogue. The default image credentials are
/// `postgres:postgres` / database `postgres`.
pub async fn postgres_ready(migrations_dir: &str) -> String {
    let container = POSTGRES
        .get_or_init(|| async {
            Postgres::default()
                .start()
                .await
                .expect("failed to start the Postgres test container")
        })
        .await;

    let port = container
        .get_host_port_ipv4(POSTGRES_PORT)
        .await
        .expect("failed to resolve the mapped Postgres port");
    let url = format!("postgres://postgres:postgres@127.0.0.1:{port}/postgres");

    POSTGRES_MIGRATED
        .get_or_init(|| {
            let url = url.clone();
            let dir = migrations_dir.to_owned();
            async move { migrate::postgres_apply(&url, &dir).await }
        })
        .await;

    url
}
