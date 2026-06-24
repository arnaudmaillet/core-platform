use error::{AppError, Severity};
use fred::error::{Error as FredError, ErrorKind};
use http::StatusCode;
use thiserror::Error;

/// Canonical error type for every Redis operation in this crate.
///
/// Implements [`AppError`] so it propagates through the platform error pipeline
/// without loss of context. All fred driver error types convert into this type,
/// isolating callers from the driver version.
///
/// ## Error code namespace
///
/// All codes use the `RDS-` prefix to distinguish Redis errors from the
/// `DB-` (postgres) and `SDB-` (scylla) namespaces used by sibling crates,
/// enabling per-backend routing in dashboards and alerting rules.
///
/// | Range      | Category                                         |
/// |------------|--------------------------------------------------|
/// | RDS-1xxx   | Retryable transient (timeout, disconnect, I/O)   |
/// | RDS-2xxx   | Connection pool exhaustion                       |
/// | RDS-3xxx   | Authentication / ACL                             |
/// | RDS-4xxx   | Command / argument / type errors                 |
/// | RDS-5xxx   | Cluster topology errors                          |
/// | RDS-7xxx   | Sentinel bootstrap errors                        |
/// | RDS-8xxx   | Configuration / TLS / protocol                   |
/// | RDS-9xxx   | Catch-all / unknown                              |
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum RedisStorageError {
    // ── Retryable transient errors (RDS-1xxx) ─────────────────────────────────

    /// A command did not receive a response within the configured timeout.
    /// Safe to retry — the command may not have been processed by the server.
    #[error("redis command timed out: {message}")]
    Timeout { message: String },

    /// The client could not route the command to a server (connection lost,
    /// cluster topology change, or no reachable node).
    /// Fred will attempt to reconnect per the configured backoff policy.
    #[error("redis connection lost or routing failed: {message}")]
    Disconnected { message: String },

    /// A low-level TCP I/O error occurred during command transmission or
    /// response reading. Usually indicates a network partition or OS resource
    /// exhaustion.
    #[error("redis I/O error: {message}")]
    Io { message: String },

    /// The client's internal command queue is full. The caller should apply
    /// backpressure and retry after a brief delay.
    #[error("redis client backpressure limit reached; retry with delay")]
    Backpressure,

    /// An in-flight command was cancelled, typically because the client was
    /// shutting down or the connection was reset mid-flight.
    #[error("redis command was cancelled")]
    Canceled,

    // ── Connection pool errors (RDS-2xxx) ──────────────────────────────────────

    /// The connection pool could not provide a connection within the configured
    /// acquire timeout. Indicates the pool is saturated; retry with backoff.
    #[error("redis connection pool exhausted: {message}")]
    PoolExhausted { message: String },

    // ── Authentication / ACL (RDS-3xxx) ───────────────────────────────────────

    /// The server rejected the configured credentials (`AUTH` command failed)
    /// or the ACL does not permit the requested operation.
    ///
    /// This is always a misconfiguration, never a user-facing auth failure.
    /// Not retryable without fixing the credentials.
    #[error("redis authentication failed: {message}")]
    Authentication { message: String },

    // ── Command / argument / type errors (RDS-4xxx) ────────────────────────────

    /// The command was applied to a key holding a value of an incompatible type
    /// (e.g., `INCR` on a string that is not an integer).
    #[error("redis wrong type operation: {message}")]
    WrongType { message: String },

    /// The command received an argument that violates its contract (e.g., a
    /// negative count, an out-of-range bit offset).
    #[error("redis invalid argument: {message}")]
    InvalidArgument { message: String },

    /// The command name itself is invalid or not supported by this server.
    #[error("redis invalid command: {message}")]
    InvalidCommand { message: String },

    /// The requested key does not exist. Surfaced only for commands where
    /// absence is an error rather than a normal empty result.
    #[error("redis key not found")]
    NotFound,

    // ── Cluster topology errors (RDS-5xxx) ─────────────────────────────────────

    /// The cluster is in a state that prevents serving the request
    /// (CLUSTERDOWN, MOVED/ASK redirect limit exceeded, hash-slot not covered).
    ///
    /// Retryable after a brief pause — the cluster may be recovering from a
    /// failover or rebalance.
    #[error("redis cluster error: {message}")]
    Cluster { message: String },

    // ── Sentinel bootstrap errors (RDS-7xxx) ───────────────────────────────────

    /// A Sentinel-specific error: no sentinel could be reached, or no primary
    /// was found for the configured service name.
    ///
    /// Retryable when sentinels are momentarily unreachable during failover.
    #[error("redis sentinel error: {message}")]
    Sentinel { message: String },

    // ── Configuration / TLS / protocol (RDS-8xxx) ──────────────────────────────

    /// A static configuration error (e.g., invalid URL, unrecognised topology,
    /// missing required parameter). Not retryable without fixing the config.
    #[error("redis configuration error: {message}")]
    Configuration { message: String },

    /// A TLS handshake or certificate validation failure.
    /// Not retryable without correcting the TLS configuration.
    #[error("redis TLS error: {message}")]
    Tls { message: String },

    /// The server returned a response that does not conform to the RESP
    /// protocol. Usually a driver bug or a serious server-side fault.
    #[error("redis protocol error: {message}")]
    Protocol { message: String },

    /// The driver could not deserialise the server's response into the
    /// expected Rust type. Indicates a type mismatch in the caller's code.
    #[error("redis parse error: {message}")]
    Parse { message: String },

    // ── Catch-all (RDS-9xxx) ──────────────────────────────────────────────────

    /// Any fred error not covered by a dedicated variant.
    #[error("redis error: {message}")]
    Unknown { message: String },
}

// ─────────────────────────────────────────────────────────────────────────────
// AppError implementation
// ─────────────────────────────────────────────────────────────────────────────

impl AppError for RedisStorageError {
    fn error_code(&self) -> &'static str {
        match self {
            RedisStorageError::Timeout { .. }        => "RDS-1001",
            RedisStorageError::Disconnected { .. }   => "RDS-1002",
            RedisStorageError::Io { .. }             => "RDS-1003",
            RedisStorageError::Backpressure          => "RDS-1004",
            RedisStorageError::Canceled              => "RDS-1005",
            RedisStorageError::PoolExhausted { .. }  => "RDS-2001",
            RedisStorageError::Authentication { .. } => "RDS-3001",
            RedisStorageError::WrongType { .. }      => "RDS-4001",
            RedisStorageError::InvalidArgument { .. }=> "RDS-4002",
            RedisStorageError::InvalidCommand { .. } => "RDS-4003",
            RedisStorageError::NotFound              => "RDS-4004",
            RedisStorageError::Cluster { .. }        => "RDS-5001",
            RedisStorageError::Sentinel { .. }       => "RDS-7001",
            RedisStorageError::Configuration { .. }  => "RDS-8001",
            RedisStorageError::Tls { .. }            => "RDS-8002",
            RedisStorageError::Protocol { .. }       => "RDS-8003",
            RedisStorageError::Parse { .. }          => "RDS-8004",
            RedisStorageError::Unknown { .. }        => "RDS-9000",
        }
    }

    fn http_status(&self) -> StatusCode {
        match self {
            RedisStorageError::Timeout { .. }
            | RedisStorageError::Disconnected { .. }
            | RedisStorageError::Io { .. }
            | RedisStorageError::Backpressure
            | RedisStorageError::Canceled
            | RedisStorageError::PoolExhausted { .. }
            | RedisStorageError::Cluster { .. }
            | RedisStorageError::Sentinel { .. }     => StatusCode::SERVICE_UNAVAILABLE,

            RedisStorageError::Authentication { .. }
            | RedisStorageError::Configuration { .. }
            | RedisStorageError::Tls { .. }
            | RedisStorageError::Protocol { .. }
            | RedisStorageError::InvalidCommand { .. }
            | RedisStorageError::Parse { .. }
            | RedisStorageError::Unknown { .. }      => StatusCode::INTERNAL_SERVER_ERROR,

            RedisStorageError::WrongType { .. }
            | RedisStorageError::InvalidArgument { .. } => StatusCode::UNPROCESSABLE_ENTITY,

            RedisStorageError::NotFound              => StatusCode::NOT_FOUND,
        }
    }

    fn severity(&self) -> Severity {
        match self {
            RedisStorageError::Authentication { .. }
            | RedisStorageError::Configuration { .. }
            | RedisStorageError::Tls { .. }
            | RedisStorageError::Protocol { .. }     => Severity::Critical,

            RedisStorageError::Timeout { .. }
            | RedisStorageError::Disconnected { .. }
            | RedisStorageError::Io { .. }
            | RedisStorageError::Backpressure
            | RedisStorageError::PoolExhausted { .. }
            | RedisStorageError::Cluster { .. }
            | RedisStorageError::Sentinel { .. }     => Severity::High,

            RedisStorageError::Canceled
            | RedisStorageError::InvalidCommand { .. }
            | RedisStorageError::Parse { .. }
            | RedisStorageError::Unknown { .. }      => Severity::Medium,

            RedisStorageError::WrongType { .. }
            | RedisStorageError::InvalidArgument { .. }
            | RedisStorageError::NotFound            => Severity::Low,
        }
    }

    fn is_retryable(&self) -> bool {
        matches!(
            self,
            RedisStorageError::Timeout { .. }
                | RedisStorageError::Disconnected { .. }
                | RedisStorageError::Io { .. }
                | RedisStorageError::Backpressure
                | RedisStorageError::Canceled
                | RedisStorageError::PoolExhausted { .. }
                | RedisStorageError::Cluster { .. }
                | RedisStorageError::Sentinel { .. }
        )
    }

    fn category(&self) -> &'static str {
        "RDS"
    }

    fn user_facing_message(&self) -> &'static str {
        match self {
            RedisStorageError::Timeout { .. }
            | RedisStorageError::Disconnected { .. }
            | RedisStorageError::Io { .. }
            | RedisStorageError::Backpressure
            | RedisStorageError::Canceled
            | RedisStorageError::PoolExhausted { .. }
            | RedisStorageError::Cluster { .. }
            | RedisStorageError::Sentinel { .. }     =>
                "The service is temporarily unavailable. Please retry.",

            RedisStorageError::NotFound              =>
                "The requested resource was not found.",

            RedisStorageError::WrongType { .. }
            | RedisStorageError::InvalidArgument { .. } =>
                "The request contains an invalid value.",

            _                                        =>
                "A cache error occurred. Please try again later.",
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// From<fred::error::Error>
// ─────────────────────────────────────────────────────────────────────────────

impl From<FredError> for RedisStorageError {
    fn from(err: FredError) -> Self {
        let message = err.to_string();
        match err.kind() {
            ErrorKind::Timeout          => RedisStorageError::Timeout      { message },
            // fred uses `Routing` for "can't find/reach a server" — maps to our Disconnected
            ErrorKind::Routing          => RedisStorageError::Disconnected  { message },
            ErrorKind::IO               => RedisStorageError::Io            { message },
            ErrorKind::Backpressure     => RedisStorageError::Backpressure,
            ErrorKind::Canceled         => RedisStorageError::Canceled,
            ErrorKind::Auth             => RedisStorageError::Authentication { message },
            ErrorKind::Cluster          => RedisStorageError::Cluster       { message },
            ErrorKind::Sentinel         => RedisStorageError::Sentinel      { message },
            // Config and Url are both static misconfigurations
            ErrorKind::Config
            | ErrorKind::Url            => RedisStorageError::Configuration { message },
            ErrorKind::Protocol         => RedisStorageError::Protocol      { message },
            ErrorKind::Parse            => RedisStorageError::Parse         { message },
            ErrorKind::InvalidArgument  => RedisStorageError::InvalidArgument { message },
            ErrorKind::InvalidCommand   => RedisStorageError::InvalidCommand { message },
            ErrorKind::NotFound         => RedisStorageError::NotFound,
            _                           => RedisStorageError::Unknown       { message },
        }
    }
}
