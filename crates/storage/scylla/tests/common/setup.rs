use std::sync::OnceLock;

use scylla_storage::{ScyllaClient, ScyllaSessionBuilder, config::ScyllaConfig};

static TRACING: OnceLock<()> = OnceLock::new();

/// Initialises a `tracing-subscriber` for test output exactly once per
/// process. Safe to call from every test; subsequent calls are no-ops.
pub fn init_tracing() {
    TRACING.get_or_init(|| {
        tracing_subscriber::fmt()
            .with_test_writer()
            .with_env_filter("scylla_storage=debug,scylla=info")
            .try_init()
            .ok();
    });
}

/// Builds a [`ScyllaClient`] for integration tests.
///
/// Reads connection parameters from the environment:
/// - `SCYLLA_CONTACT_POINTS` (default: `127.0.0.1:9042`)
/// - `SCYLLA_LOCAL_DC` (default: `datacenter1`)
/// - `SCYLLA_KEYSPACE` — optional; leave unset to skip `USE KEYSPACE`
///
/// Mark every test that calls this with `#[ignore]` so that CI skips it
/// unless a live cluster is available (run with `cargo test -- --ignored`).
#[allow(dead_code)]
pub async fn test_client() -> ScyllaClient {
    init_tracing();
    let config = ScyllaConfig::from_env();
    ScyllaSessionBuilder::new(config)
        .build()
        .await
        .expect("integration test: failed to build ScyllaClient")
}
