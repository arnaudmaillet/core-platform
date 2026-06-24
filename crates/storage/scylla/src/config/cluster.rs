use std::time::Duration;

/// Wire-protocol compression applied to all connections in the session.
#[derive(Debug, Clone, Copy, Default)]
pub enum CompressionKind {
    /// No compression. Lowest CPU, highest bandwidth.
    None,
    /// LZ4 block compression. Best CPU/bandwidth trade-off for mixed workloads.
    #[default]
    Lz4,
    /// Snappy compression. Similar trade-off to LZ4; prefer for CPU-bound nodes.
    Snappy,
}

/// Complete configuration surface for a ScyllaDB cluster connection.
///
/// Construct via [`ScyllaConfig::from_env`] for production, or build the struct
/// directly in tests and local development.
#[derive(Debug, Clone)]
pub struct ScyllaConfig {
    /// Comma-separated list of `host:port` contact points used for cluster
    /// bootstrap topology discovery.
    /// `SCYLLA_CONTACT_POINTS` (default: `"127.0.0.1:9042"`)
    pub contact_points: Vec<String>,

    /// Datacenter name this process is co-located with. Used by the
    /// token-aware + DC-aware `DefaultPolicy` to route queries locally.
    /// Must exactly match the `local_dc` name reported by `system.local`.
    /// `SCYLLA_LOCAL_DC` (default: `"datacenter1"`)
    pub local_dc: String,

    /// Keyspace sent to the cluster on session open (`USE <keyspace>`).
    /// Leave as `None` to skip — callers can fully-qualify table names.
    /// `SCYLLA_KEYSPACE`
    pub keyspace: Option<String>,

    /// Plaintext authentication username.
    /// `SCYLLA_USERNAME`
    pub username: Option<String>,

    /// Plaintext authentication password.
    /// `SCYLLA_PASSWORD`
    pub password: Option<String>,

    /// Wire-protocol compression applied to all connections.
    /// `SCYLLA_COMPRESSION` (`lz4` | `snappy` | `none`, default: `lz4`)
    pub compression: CompressionKind,

    /// Maximum time allowed for the driver to establish the initial TCP+CQL
    /// handshake with each contact point.
    /// `SCYLLA_CONNECT_TIMEOUT_SECS` (default: 5)
    pub connect_timeout: Duration,

    /// Default per-request timeout applied through the `Strict` execution
    /// profile. Individual profiles may override this.
    /// `SCYLLA_REQUEST_TIMEOUT_SECS` (default: 5)
    pub request_timeout: Duration,

    /// Maximum number of prepared-statement entries held in the
    /// `CachingSession` LRU cache. Each entry avoids a `PREPARE` round-trip.
    /// `SCYLLA_STATEMENT_CACHE_CAPACITY` (default: 256)
    pub statement_cache_capacity: usize,
}

impl Default for ScyllaConfig {
    fn default() -> Self {
        Self {
            contact_points: vec!["127.0.0.1:9042".to_string()],
            local_dc: "datacenter1".to_string(),
            keyspace: None,
            username: None,
            password: None,
            compression: CompressionKind::Lz4,
            connect_timeout: Duration::from_secs(5),
            request_timeout: Duration::from_secs(5),
            statement_cache_capacity: 256,
        }
    }
}

impl ScyllaConfig {
    /// Populates every field from environment variables, falling back to
    /// production-safe defaults when variables are absent.
    ///
    /// Panics only if `SCYLLA_CONTACT_POINTS` is set to an empty string — every
    /// other variable has a sensible default.
    pub fn from_env() -> Self {
        let contact_points = std::env::var("SCYLLA_CONTACT_POINTS")
            .unwrap_or_else(|_| "127.0.0.1:9042".to_string())
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>();

        assert!(!contact_points.is_empty(), "SCYLLA_CONTACT_POINTS must not be empty");

        let local_dc = std::env::var("SCYLLA_LOCAL_DC")
            .unwrap_or_else(|_| "datacenter1".to_string());

        let keyspace = std::env::var("SCYLLA_KEYSPACE").ok();
        let username = std::env::var("SCYLLA_USERNAME").ok();
        let password = std::env::var("SCYLLA_PASSWORD").ok();

        let compression = match std::env::var("SCYLLA_COMPRESSION").as_deref() {
            Ok("snappy") => CompressionKind::Snappy,
            Ok("none")   => CompressionKind::None,
            _            => CompressionKind::Lz4,
        };

        Self {
            contact_points,
            local_dc,
            keyspace,
            username,
            password,
            compression,
            connect_timeout: Duration::from_secs(parse_env("SCYLLA_CONNECT_TIMEOUT_SECS", 5)),
            request_timeout: Duration::from_secs(parse_env("SCYLLA_REQUEST_TIMEOUT_SECS", 5)),
            statement_cache_capacity: parse_env("SCYLLA_STATEMENT_CACHE_CAPACITY", 256),
        }
    }
}

fn parse_env<T>(key: &str, default: T) -> T
where
    T: std::str::FromStr,
    T::Err: std::fmt::Debug,
{
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}
