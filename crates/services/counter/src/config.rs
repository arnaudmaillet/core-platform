//! Environment-sourced configuration, resolved once at boot and threaded into the
//! composition root ([`crate::app`]). Each backend's connection config comes from
//! its own `from_env`; counter-specific knobs are read here.

use std::time::Duration;

use postgres_storage::PostgresConfig;
use redis_storage::RedisConfig;
use scylla_storage::ScyllaConfig;
use transport::kafka::config::KafkaClientConfig;

use crate::domain::WindowSize;

const DEFAULT_WINDOW_MS: u64 = 5_000;
const DEFAULT_POPULARITY_INTERVAL_S: u64 = 60;
const DEFAULT_READ_TIMEOUT_MS: u64 = 50;
const DEFAULT_RECONCILE_INTERVAL_S: u64 = 3_600;
const DEFAULT_DRIFT_TOLERANCE: i64 = 5;

/// Fully-resolved counter configuration shared by both binaries (the read server
/// uses the storage configs; the worker additionally uses Kafka + the windowing
/// knobs).
pub struct CounterConfig {
    pub postgres: PostgresConfig,
    pub redis: RedisConfig,
    pub scylla: ScyllaConfig,
    pub kafka: KafkaClientConfig,
    /// Tumbling pre-aggregation window — the N→1 collapse width.
    pub aggregation_window: WindowSize,
    /// How often the worker drains closed windows and flushes them.
    pub flush_interval: Duration,
    /// Slow-loop cadence for publishing the coarse popularity signal.
    pub popularity_interval: Duration,
    /// Hard per-request hot-read timeout; on elapse the read fails open (stale).
    pub read_timeout: Duration,
    /// Cadence of the reconciliation sweep loop.
    pub reconcile_interval: Duration,
    /// Absolute drift tolerated before reconciliation corrects an exact counter.
    pub drift_tolerance: i64,
    /// gRPC endpoint of `social-graph` — the authoritative source for
    /// follower/following counts the reconciliation loop queries.
    pub social_graph_endpoint: String,
}

impl CounterConfig {
    pub fn from_env() -> Self {
        let window_ms = env_u64("COUNTER_AGGREGATION_WINDOW_MS", DEFAULT_WINDOW_MS);
        // A zero window would be rejected by the VO; fall back to the safe default.
        let aggregation_window =
            WindowSize::from_millis(window_ms).unwrap_or_else(|_| safe_window());

        let flush_ms = env_u64("COUNTER_FLUSH_INTERVAL_MS", window_ms);

        Self {
            postgres: PostgresConfig::from_env(),
            redis: RedisConfig::from_env(),
            scylla: ScyllaConfig::from_env(),
            kafka: KafkaClientConfig::from_env(),
            aggregation_window,
            flush_interval: Duration::from_millis(flush_ms),
            popularity_interval: Duration::from_secs(env_u64(
                "COUNTER_POPULARITY_INTERVAL_S",
                DEFAULT_POPULARITY_INTERVAL_S,
            )),
            read_timeout: Duration::from_millis(env_u64(
                "COUNTER_READ_TIMEOUT_MS",
                DEFAULT_READ_TIMEOUT_MS,
            )),
            reconcile_interval: Duration::from_secs(env_u64(
                "COUNTER_RECONCILE_INTERVAL_S",
                DEFAULT_RECONCILE_INTERVAL_S,
            )),
            drift_tolerance: std::env::var("COUNTER_DRIFT_TOLERANCE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(DEFAULT_DRIFT_TOLERANCE),
            social_graph_endpoint: std::env::var("COUNTER_SOCIAL_GRAPH_GRPC_ENDPOINT")
                .unwrap_or_else(|_| "http://localhost:50053".to_owned()),
        }
    }
}

fn safe_window() -> WindowSize {
    WindowSize::from_millis(DEFAULT_WINDOW_MS).expect("default window is non-zero")
}

fn env_u64(key: &str, default: u64) -> u64 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}
