//! Multi-infra orchestration: ephemeral ScyllaDB, Redis, and Kafka containers.
//!
//! # Design pillars (mirrors the workspace `transport` live-broker harness)
//!
//! - **One container set per test binary.** Each backend is booted lazily through
//!   a [`OnceCell`] and shared by every scenario in the binary. Kafka boots only
//!   when a scenario that needs it first asks — scenarios 1–3 never pay for it.
//! - **Zero port conflicts.** Every endpoint is resolved from the OS-assigned
//!   mapped host port via `get_host_port_ipv4`; nothing is statically bound.
//! - **Isolation by namespacing, not teardown.** Scenarios mint fresh
//!   `conversation_id`/`profile_id` UUIDs; ScyllaDB partitions and all Redis keys
//!   are keyed by `conversation_id`, so concurrent scenarios never collide and the
//!   suite runs in parallel. Kafka topics/groups are UUID-suffixed.
//! - **Migrations applied exactly once**, behind a [`OnceCell`], so parallel
//!   scenarios never race a concurrent `CREATE`.
//!
//! ## Redis image override
//!
//! The `testcontainers-modules` redis module defaults to `redis:5.0`, which
//! predates sharded pub/sub (`SSUBSCRIBE`/`SPUBLISH`/`SUNSUBSCRIBE`, Redis 7.0).
//! The chat routing layer depends on those, so we pin a 7.x image explicitly.

use std::time::Duration;

use rdkafka::admin::{AdminClient, AdminOptions, NewTopic, TopicReplication};
use rdkafka::client::DefaultClientContext;
use rdkafka::config::ClientConfig;
use testcontainers::core::WaitFor;
use testcontainers::runners::AsyncRunner;
use testcontainers::{ContainerAsync, GenericImage, ImageExt};
use testcontainers_modules::kafka::apache::{Kafka, KAFKA_PORT};
use testcontainers_modules::scylladb::ScyllaDB;
use tokio::sync::OnceCell;

use super::migrate;

/// Internal Redis port; the 7.x image exposes it and testcontainers maps it.
const REDIS_PORT: u16 = 6379;
/// Internal ScyllaDB CQL port.
const SCYLLA_CQL_PORT: u16 = 9042;

static SCYLLA: OnceCell<ContainerAsync<ScyllaDB>> = OnceCell::const_new();
static REDIS: OnceCell<ContainerAsync<GenericImage>> = OnceCell::const_new();
static KAFKA: OnceCell<ContainerAsync<Kafka>> = OnceCell::const_new();
static MIGRATED: OnceCell<()> = OnceCell::const_new();

/// Boots ScyllaDB (once), applies the six `.cql` migrations (once), and returns
/// the contact point. `--developer-mode 1 --smp 1` makes a single-node boot fast
/// and reliable on untuned hosts (CI / macOS).
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
    let contact_point = format!("127.0.0.1:{port}");

    MIGRATED
        .get_or_init(|| migrate::apply_all(&contact_point))
        .await;

    contact_point
}

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
        // A topic that already exists from a previous scenario is fine.
        if let Err((topic, code)) = result
            && code != rdkafka::types::RDKafkaErrorCode::TopicAlreadyExists
        {
            panic!("failed to create topic '{topic}': {code}");
        }
    }
}
