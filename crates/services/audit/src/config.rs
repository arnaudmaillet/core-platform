//! Environment-sourced configuration, resolved once at boot and threaded into the
//! composition root ([`crate::app`]). Each backend's connection config comes from
//! its own `from_env`; audit-specific knobs are read here.

use std::time::Duration;

use postgres_storage::PostgresConfig;
use transport::kafka::config::KafkaClientConfig;

use crate::infrastructure::ObjectLockConfig;

const DEFAULT_RECORD_TIMEOUT_MS: u64 = 2_000;
const DEFAULT_CHECKPOINT_INTERVAL_S: u64 = 300;
const DEFAULT_PRESIGN_TTL_S: u64 = 900;
const DEFAULT_OBJECT_TIMEOUT_MS: u64 = 5_000;

/// Fully-resolved audit configuration shared by both binaries (the server uses the
/// ledger/archive/vault/anchor + the sync deadline; the worker additionally uses
/// Kafka + the checkpoint cadence).
pub struct AuditConfig {
    pub postgres: PostgresConfig,
    pub kafka: KafkaClientConfig,
    pub object_lock: ObjectLockConfig,
    /// The synchronous `RecordPrivileged` durable-commit deadline. On elapse the
    /// lane fails CLOSED (`AUD-4004`) and the caller must deny the action.
    pub record_timeout: Duration,
    /// How often the worker snapshots the partition heads into an anchored
    /// Merkle checkpoint.
    pub checkpoint_interval: Duration,
}

impl AuditConfig {
    pub fn from_env() -> Self {
        Self {
            postgres: PostgresConfig::from_env(),
            kafka: KafkaClientConfig::from_env(),
            object_lock: object_lock_from_env(),
            record_timeout: Duration::from_millis(env_u64(
                "AUDIT_RECORD_TIMEOUT_MS",
                DEFAULT_RECORD_TIMEOUT_MS,
            )),
            checkpoint_interval: Duration::from_secs(env_u64(
                "AUDIT_CHECKPOINT_INTERVAL_S",
                DEFAULT_CHECKPOINT_INTERVAL_S,
            )),
        }
    }
}

fn object_lock_from_env() -> ObjectLockConfig {
    ObjectLockConfig {
        endpoint: env_str("AUDIT_OBJECT_STORE_ENDPOINT", "http://localhost:9000"),
        region: env_str("AUDIT_OBJECT_STORE_REGION", "us-east-1"),
        bucket: env_str("AUDIT_OBJECT_STORE_BUCKET", "audit-archive"),
        access_key: env_str("AUDIT_OBJECT_STORE_ACCESS_KEY", "minioadmin"),
        secret_key: env_str("AUDIT_OBJECT_STORE_SECRET_KEY", "minioadmin"),
        presign_ttl: Duration::from_secs(env_u64("AUDIT_OBJECT_PRESIGN_TTL_S", DEFAULT_PRESIGN_TTL_S)),
        request_timeout: Duration::from_millis(env_u64(
            "AUDIT_OBJECT_TIMEOUT_MS",
            DEFAULT_OBJECT_TIMEOUT_MS,
        )),
    }
}

fn env_u64(key: &str, default: u64) -> u64 {
    std::env::var(key).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

fn env_str(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_owned())
}
