use error::{AppError, Severity};
use http::StatusCode;
use scylla::errors::{DbError, ExecutionError, NewSessionError, RequestAttemptError};
use thiserror::Error;

/// Canonical error type for every ScyllaDB operation in this crate.
///
/// Implements [`AppError`] so it propagates through the platform error pipeline
/// without loss of context. All scylla driver error types (`ExecutionError`,
/// `NewSessionError`) convert into this type, isolating callers from the driver
/// version.
///
/// ## Error code namespace
///
/// All codes use the `SDB-` prefix (distinct from `DB-` used by the postgres
/// storage crate) so dashboards and alerting rules can route errors to the
/// correct storage backend.
///
/// | Range    | Category                     |
/// |----------|------------------------------|
/// | SDB-1xxx | Retryable transient errors   |
/// | SDB-2xxx | Connection pool              |
/// | SDB-3xxx | Authentication / access      |
/// | SDB-4xxx | Schema / idempotency         |
/// | SDB-5xxx | Query / CQL errors           |
/// | SDB-6xxx | Cluster state / replication  |
/// | SDB-7xxx | Session bootstrap            |
/// | SDB-8xxx | Configuration / protocol     |
/// | SDB-9xxx | Catch-all / unknown          |
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ScyllaStorageError {
    // ── Retryable transient errors (SDB-1xxx) ─────────────────────────────

    /// Write was not acknowledged by enough replicas within the timeout.
    /// Safe to retry — the driver's `DefaultRetryPolicy` already attempted
    /// at least once; the `resilience` crate retry layer should be used for
    /// application-level retry.
    #[error("write timed out at {consistency} (received {received}/{required} replicas)")]
    WriteTimeout { consistency: String, received: i32, required: i32 },

    /// Read did not receive enough replica responses within the timeout.
    #[error("read timed out at {consistency} (received {received}/{required} replicas)")]
    ReadTimeout { consistency: String, received: i32, required: i32 },

    /// Not enough replicas are alive to satisfy the requested consistency level.
    #[error("cluster unavailable at {consistency} (required {required}, alive {alive})")]
    Unavailable { consistency: String, required: i32, alive: i32 },

    /// A coordinator node reported it is temporarily overloaded.
    #[error("coordinator node is overloaded; retry with backoff")]
    Overloaded,

    /// ScyllaDB-specific: coordinator rejected the request due to per-partition
    /// rate limiting. Safe to retry with exponential backoff.
    #[error("rate limit reached on the coordinator; retry with backoff")]
    RateLimitReached,

    /// The targeted node is still joining the ring. The driver will retry on
    /// another node; this variant surfaces only when all attempts fail.
    #[error("node is bootstrapping; retry on another node")]
    IsBootstrapping,

    /// The client-side timeout (configured via `request_timeout`) fired before
    /// the driver received a response. The operation may or may not have been
    /// applied on the server.
    #[error("client-side request timeout after {millis}ms")]
    ClientTimeout { millis: u128 },

    // ── Connection pool (SDB-2xxx) ─────────────────────────────────────────

    /// No live connection could be obtained from the pool.
    #[error("connection pool exhausted or unavailable: {message}")]
    ConnectionPool { message: String },

    /// A transport-level error occurred during request transmission (broken
    /// connection, stream exhaustion, unexpected response).
    #[error("transport error: {message}")]
    Transport { message: String },

    // ── Authentication / access control (SDB-3xxx) ────────────────────────

    /// The cluster rejected the configured credentials. Indicates a
    /// misconfiguration — not a user-visible auth failure.
    #[error("authentication failed: {message}")]
    AuthenticationError { message: String },

    /// The authenticated role lacks permission for the requested operation.
    #[error("unauthorized: {message}")]
    Unauthorized { message: String },

    // ── Schema / idempotency (SDB-4xxx) ───────────────────────────────────

    /// A CREATE TABLE / CREATE KEYSPACE / CREATE TYPE attempted to create an
    /// object that already exists.
    #[error("object already exists in keyspace '{keyspace}', table '{table}'")]
    AlreadyExists { keyspace: String, table: String },

    // ── Query / CQL errors (SDB-5xxx) ─────────────────────────────────────

    /// The CQL statement is syntactically invalid or failed during preparation.
    /// Indicates a programming error — not retryable.
    #[error("bad CQL query: {message}")]
    BadQuery { message: String },

    /// The CQL statement is syntactically valid but semantically rejected by
    /// the server (wrong type, unknown column, etc.).
    #[error("invalid query: {message}")]
    QueryInvalid { message: String },

    /// A write was acknowledged by the coordinator but failed on one or more
    /// replicas. The data may be in a partially-replicated state.
    /// Not safe to retry blindly — use idempotency guards.
    #[error("write failed on {numfailures} replica(s); data may be partially replicated")]
    WriteFailure { numfailures: i32 },

    /// A read could not be satisfied because one or more replicas returned
    /// errors instead of responses.
    #[error("read failed on {numfailures} replica(s)")]
    ReadFailure { numfailures: i32 },

    // ── Cluster state / replication (SDB-6xxx) ────────────────────────────

    /// Schema agreement could not be reached across the cluster. DDL changes
    /// may be incomplete.
    #[error("schema agreement failed: {message}")]
    SchemaConflict { message: String },

    // ── Session bootstrap (SDB-7xxx) ──────────────────────────────────────

    /// Session bootstrap failed: contact points unreachable or metadata
    /// unavailable.
    #[error("session bootstrap failed: {message}")]
    Bootstrap { message: String },

    // ── Configuration / protocol (SDB-8xxx) ───────────────────────────────

    /// A static configuration error (empty contact points list, invalid
    /// keyspace, load-balancing policy returned an empty plan).
    #[error("storage configuration error: {message}")]
    Configuration { message: String },

    /// A low-level CQL protocol error. Indicates a driver bug or an
    /// incompatible server version.
    #[error("protocol error: {message}")]
    ProtocolError { message: String },

    // ── Catch-all (SDB-9xxx) ──────────────────────────────────────────────

    /// Any server error not covered by a dedicated variant.
    /// `code` is the raw CQL error code in hex (e.g. `"0x0000"`).
    #[error("database error [{code}]: {message}")]
    Unknown { code: String, message: String },
}

// ─────────────────────────────────────────────────────────────────────────────
// AppError implementation
// ─────────────────────────────────────────────────────────────────────────────

impl AppError for ScyllaStorageError {
    fn error_code(&self) -> &'static str {
        match self {
            ScyllaStorageError::WriteTimeout { .. }      => "SDB-1001",
            ScyllaStorageError::ReadTimeout { .. }       => "SDB-1002",
            ScyllaStorageError::Unavailable { .. }       => "SDB-1003",
            ScyllaStorageError::Overloaded               => "SDB-1004",
            ScyllaStorageError::RateLimitReached         => "SDB-1005",
            ScyllaStorageError::IsBootstrapping          => "SDB-1006",
            ScyllaStorageError::ClientTimeout { .. }     => "SDB-1007",
            ScyllaStorageError::ConnectionPool { .. }    => "SDB-2001",
            ScyllaStorageError::Transport { .. }         => "SDB-2002",
            ScyllaStorageError::AuthenticationError { .. } => "SDB-3001",
            ScyllaStorageError::Unauthorized { .. }      => "SDB-3002",
            ScyllaStorageError::AlreadyExists { .. }     => "SDB-4001",
            ScyllaStorageError::BadQuery { .. }          => "SDB-5001",
            ScyllaStorageError::QueryInvalid { .. }      => "SDB-5002",
            ScyllaStorageError::WriteFailure { .. }      => "SDB-5003",
            ScyllaStorageError::ReadFailure { .. }       => "SDB-5004",
            ScyllaStorageError::SchemaConflict { .. }    => "SDB-6001",
            ScyllaStorageError::Bootstrap { .. }         => "SDB-7001",
            ScyllaStorageError::Configuration { .. }     => "SDB-8001",
            ScyllaStorageError::ProtocolError { .. }     => "SDB-8002",
            ScyllaStorageError::Unknown { .. }           => "SDB-9000",
        }
    }

    fn http_status(&self) -> StatusCode {
        match self {
            ScyllaStorageError::WriteTimeout { .. }
            | ScyllaStorageError::ReadTimeout { .. }
            | ScyllaStorageError::Unavailable { .. }
            | ScyllaStorageError::Overloaded
            | ScyllaStorageError::RateLimitReached
            | ScyllaStorageError::IsBootstrapping
            | ScyllaStorageError::ClientTimeout { .. }
            | ScyllaStorageError::ConnectionPool { .. }
            | ScyllaStorageError::Transport { .. }
            | ScyllaStorageError::WriteFailure { .. }
            | ScyllaStorageError::ReadFailure { .. }
            | ScyllaStorageError::SchemaConflict { .. }
            | ScyllaStorageError::Bootstrap { .. }    => StatusCode::SERVICE_UNAVAILABLE,

            ScyllaStorageError::Unauthorized { .. }   => StatusCode::FORBIDDEN,
            ScyllaStorageError::AlreadyExists { .. }  => StatusCode::CONFLICT,
            ScyllaStorageError::QueryInvalid { .. }   => StatusCode::UNPROCESSABLE_ENTITY,

            ScyllaStorageError::AuthenticationError { .. }
            | ScyllaStorageError::BadQuery { .. }
            | ScyllaStorageError::Configuration { .. }
            | ScyllaStorageError::ProtocolError { .. }
            | ScyllaStorageError::Unknown { .. }      => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn severity(&self) -> Severity {
        match self {
            ScyllaStorageError::Unavailable { .. }
            | ScyllaStorageError::AuthenticationError { .. }
            | ScyllaStorageError::Bootstrap { .. }
            | ScyllaStorageError::Configuration { .. }
            | ScyllaStorageError::ProtocolError { .. }
            | ScyllaStorageError::BadQuery { .. }     => Severity::Critical,

            ScyllaStorageError::WriteTimeout { .. }
            | ScyllaStorageError::ReadTimeout { .. }
            | ScyllaStorageError::Overloaded
            | ScyllaStorageError::RateLimitReached
            | ScyllaStorageError::ClientTimeout { .. }
            | ScyllaStorageError::ConnectionPool { .. }
            | ScyllaStorageError::Transport { .. }
            | ScyllaStorageError::WriteFailure { .. }
            | ScyllaStorageError::ReadFailure { .. }
            | ScyllaStorageError::SchemaConflict { .. } => Severity::High,

            ScyllaStorageError::IsBootstrapping
            | ScyllaStorageError::QueryInvalid { .. }
            | ScyllaStorageError::Unknown { .. }      => Severity::Medium,

            ScyllaStorageError::Unauthorized { .. }
            | ScyllaStorageError::AlreadyExists { .. } => Severity::Low,
        }
    }

    fn is_retryable(&self) -> bool {
        matches!(
            self,
            ScyllaStorageError::WriteTimeout { .. }
                | ScyllaStorageError::ReadTimeout { .. }
                | ScyllaStorageError::Unavailable { .. }
                | ScyllaStorageError::Overloaded
                | ScyllaStorageError::RateLimitReached
                | ScyllaStorageError::IsBootstrapping
                | ScyllaStorageError::ClientTimeout { .. }
                | ScyllaStorageError::ConnectionPool { .. }
                | ScyllaStorageError::Transport { .. }
        )
    }

    fn category(&self) -> &'static str {
        "SDB"
    }

    fn user_facing_message(&self) -> &'static str {
        match self {
            ScyllaStorageError::WriteTimeout { .. }
            | ScyllaStorageError::ReadTimeout { .. }
            | ScyllaStorageError::Unavailable { .. }
            | ScyllaStorageError::Overloaded
            | ScyllaStorageError::RateLimitReached
            | ScyllaStorageError::IsBootstrapping
            | ScyllaStorageError::ClientTimeout { .. }
            | ScyllaStorageError::ConnectionPool { .. }
            | ScyllaStorageError::Transport { .. }    =>
                "The service is temporarily overloaded. Please retry.",
            ScyllaStorageError::Unauthorized { .. }   =>
                "You do not have permission to perform this operation.",
            ScyllaStorageError::AlreadyExists { .. }  =>
                "The resource already exists.",
            ScyllaStorageError::WriteFailure { .. }
            | ScyllaStorageError::ReadFailure { .. }  =>
                "A replication error occurred. Please try again later.",
            _                                         =>
                "A database error occurred. Please try again later.",
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// From<ExecutionError>
// ─────────────────────────────────────────────────────────────────────────────

impl From<ExecutionError> for ScyllaStorageError {
    fn from(err: ExecutionError) -> Self {
        match err {
            ExecutionError::LastAttemptError(attempt_err) => map_attempt_error(attempt_err),

            ExecutionError::RequestTimeout(dur) => ScyllaStorageError::ClientTimeout {
                millis: dur.as_millis(),
            },

            ExecutionError::ConnectionPoolError(e) => ScyllaStorageError::ConnectionPool {
                message: e.to_string(),
            },

            ExecutionError::BadQuery(e) => ScyllaStorageError::BadQuery {
                message: e.to_string(),
            },

            ExecutionError::PrepareError(e) => ScyllaStorageError::BadQuery {
                message: format!("statement prepare failed: {e}"),
            },

            // The load-balancing policy returned an empty routing plan. This
            // almost always means SCYLLA_LOCAL_DC does not match the cluster's
            // datacenter name.
            ExecutionError::EmptyPlan => ScyllaStorageError::Configuration {
                message: "load balancing policy returned an empty plan — \
                          verify that SCYLLA_LOCAL_DC matches the cluster's local_dc"
                    .into(),
            },

            ExecutionError::UseKeyspaceError(e) => ScyllaStorageError::Configuration {
                message: format!("USE KEYSPACE failed: {e}"),
            },

            ExecutionError::SchemaAgreementError(e) => ScyllaStorageError::SchemaConflict {
                message: e.to_string(),
            },

            ExecutionError::MetadataError(e) => ScyllaStorageError::ProtocolError {
                message: format!("cluster metadata fetch failed: {e}"),
            },

            _ => ScyllaStorageError::Unknown {
                code: String::new(),
                message: err.to_string(),
            },
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// From<NewSessionError>
// ─────────────────────────────────────────────────────────────────────────────

impl From<NewSessionError> for ScyllaStorageError {
    fn from(err: NewSessionError) -> Self {
        match err {
            NewSessionError::FailedToResolveAnyHostname(hosts) => ScyllaStorageError::Bootstrap {
                message: format!(
                    "could not resolve any contact point — checked: {:?}",
                    hosts
                ),
            },

            NewSessionError::EmptyKnownNodesList => ScyllaStorageError::Configuration {
                message: "contact points list is empty; set SCYLLA_CONTACT_POINTS".into(),
            },

            NewSessionError::MetadataError(e) => ScyllaStorageError::Bootstrap {
                message: format!("initial cluster metadata fetch failed: {e}"),
            },

            NewSessionError::UseKeyspaceError(e) => ScyllaStorageError::Configuration {
                message: format!("USE KEYSPACE failed during session bootstrap: {e}"),
            },

            _ => ScyllaStorageError::Bootstrap {
                message: err.to_string(),
            },
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Internal helpers
// ─────────────────────────────────────────────────────────────────────────────

fn map_attempt_error(err: RequestAttemptError) -> ScyllaStorageError {
    match err {
        RequestAttemptError::DbError(db_err, msg) => map_db_error(db_err, msg),

        RequestAttemptError::BrokenConnectionError(e) => ScyllaStorageError::Transport {
            message: format!("broken connection: {e}"),
        },

        RequestAttemptError::UnableToAllocStreamId => ScyllaStorageError::Transport {
            message: "unable to allocate CQL stream id; connection may be saturated".into(),
        },

        other => ScyllaStorageError::Transport {
            message: other.to_string(),
        },
    }
}

fn map_db_error(db_err: DbError, msg: String) -> ScyllaStorageError {
    match db_err {
        DbError::WriteTimeout { consistency, received, required, .. } => {
            ScyllaStorageError::WriteTimeout {
                consistency: format!("{consistency:?}"),
                received,
                required,
            }
        }

        DbError::ReadTimeout { consistency, received, required, .. } => {
            ScyllaStorageError::ReadTimeout {
                consistency: format!("{consistency:?}"),
                received,
                required,
            }
        }

        DbError::Unavailable { consistency, required, alive } => {
            ScyllaStorageError::Unavailable {
                consistency: format!("{consistency:?}"),
                required,
                alive,
            }
        }

        DbError::Overloaded       => ScyllaStorageError::Overloaded,
        DbError::IsBootstrapping  => ScyllaStorageError::IsBootstrapping,

        DbError::RateLimitReached { .. } => ScyllaStorageError::RateLimitReached,

        DbError::TruncateError => ScyllaStorageError::Transport {
            message: "TRUNCATE error; the operation may not have completed on all replicas".into(),
        },

        DbError::WriteFailure { numfailures, .. } => {
            ScyllaStorageError::WriteFailure { numfailures }
        }

        DbError::ReadFailure { numfailures, .. } => {
            ScyllaStorageError::ReadFailure { numfailures }
        }

        DbError::AuthenticationError => ScyllaStorageError::AuthenticationError { message: msg },

        DbError::Unauthorized => ScyllaStorageError::Unauthorized { message: msg },

        DbError::AlreadyExists { keyspace, table } => {
            ScyllaStorageError::AlreadyExists { keyspace, table }
        }

        DbError::SyntaxError => ScyllaStorageError::BadQuery { message: msg },

        DbError::Invalid => ScyllaStorageError::QueryInvalid { message: msg },

        DbError::ConfigError => ScyllaStorageError::Configuration { message: msg },

        DbError::FunctionFailure { keyspace, function, .. } => ScyllaStorageError::QueryInvalid {
            message: format!("user-defined function failure in {keyspace}.{function}"),
        },

        DbError::Unprepared { .. } => ScyllaStorageError::Transport {
            message: "prepared statement id unknown to coordinator; re-execute to trigger re-prepare"
                .into(),
        },

        DbError::ServerError => ScyllaStorageError::ProtocolError {
            message: format!("server error: {msg}"),
        },

        DbError::ProtocolError => ScyllaStorageError::ProtocolError {
            message: format!("CQL protocol error: {msg}"),
        },

        DbError::Other(code) => ScyllaStorageError::Unknown {
            code:    format!("0x{code:04X}"),
            message: msg,
        },

        _ => ScyllaStorageError::Unknown {
            code:    String::new(),
            message: format!("unhandled DbError: {msg}"),
        },
    }
}
