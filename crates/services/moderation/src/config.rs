//! Environment-sourced configuration for the moderation service. Resolved once at
//! boot and threaded into the composition root ([`crate::app::App::build`]).
//! Backend connection configs (Postgres / Scylla / Redis / Kafka) are resolved
//! separately via their own `from_env`.

use chrono::Duration;

use crate::application::ModerationPolicy;
use crate::domain::value_object::PolicyVersion;

/// Fully-resolved moderation configuration.
pub struct ModerationConfig {
    pub policy: ModerationPolicy,
    /// gRPC endpoint of the `account` service, e.g. `http://account:50059`.
    pub account_endpoint: String,
    /// Per-request deadline on `account` RPCs — tonic has no default timeout.
    pub account_rpc_timeout: std::time::Duration,
    /// Connect deadline when dialing the `account` channel.
    pub account_connect_timeout: std::time::Duration,
}

impl ModerationConfig {
    /// Resolves configuration from the environment. Every value has a
    /// production-shaped default; the penalty ladder defaults to
    /// [`ModerationPolicy::standard`] with the TTLs overridable.
    pub fn from_env() -> anyhow::Result<Self> {
        let mut policy = ModerationPolicy::standard();
        policy.restrict_actor_ttl =
            Duration::seconds(env_secs("MODERATION_RESTRICT_TTL_SECS", 604_800)); // 7d
        policy.suspend_ttl =
            Duration::seconds(env_secs("MODERATION_SUSPEND_TTL_SECS", 2_592_000)); // 30d
        if let Ok(v) = std::env::var("MODERATION_SCREEN_POLICY_VERSION")
            && !v.trim().is_empty()
        {
            policy.screen_policy_version = PolicyVersion::new(v)?;
        }
        policy.screen_timeout =
            std::time::Duration::from_millis(env_secs("MODERATION_SCREEN_TIMEOUT_MS", 200) as u64);

        Ok(Self {
            policy,
            account_endpoint: env_or("MODERATION_ACCOUNT_GRPC_ENDPOINT", "http://localhost:50059"),
            account_rpc_timeout: std::time::Duration::from_millis(
                env_secs("MODERATION_ACCOUNT_RPC_TIMEOUT_MS", 2_000) as u64,
            ),
            account_connect_timeout: std::time::Duration::from_millis(
                env_secs("MODERATION_ACCOUNT_CONNECT_TIMEOUT_MS", 2_000) as u64,
            ),
        })
    }
}

fn env_or(key: &str, default: impl Into<String>) -> String {
    std::env::var(key).unwrap_or_else(|_| default.into())
}

fn env_secs(key: &str, default: i64) -> i64 {
    std::env::var(key).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}
