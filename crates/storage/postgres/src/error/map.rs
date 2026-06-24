use crate::routing::ShardId;
use error::{AppError, Severity};
use http::StatusCode;
use thiserror::Error;

/// Canonical error type for every database operation in this crate.
///
/// Implements [`AppError`] so it can be wrapped in `DistributedError<StorageError>`
/// and propagate through the platform's error pipeline without loss of context.
///
/// Variants are deliberately free of `sqlx` types — the conversion happens in
/// [`From<sqlx::Error>`] and callers never depend on sqlx directly.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum StorageError {
    /// PostgreSQL error code `23505`.
    /// A row with this value already exists for the named constraint.
    #[error("unique constraint violated on '{constraint}'")]
    UniqueViolation { constraint: String },

    /// PostgreSQL error code `23503`.
    /// The referenced parent row does not exist in the parent table.
    #[error("foreign key constraint violated on '{constraint}'")]
    ForeignKeyViolation { constraint: String },

    /// PostgreSQL error code `23502`.
    /// A non-null column received a NULL value. `detail` contains the
    /// PostgreSQL error message, which includes the column name.
    #[error("not-null constraint violated: {detail}")]
    NotNullViolation { detail: String },

    /// PostgreSQL error code `23514`.
    /// A `CHECK` expression evaluated to false for the named constraint.
    #[error("check constraint violated on '{constraint}'")]
    CheckViolation { constraint: String },

    /// PostgreSQL error code `40P01`.
    /// The database rolled back this transaction to resolve a deadlock cycle.
    /// Safe to retry with exponential back-off.
    #[error("deadlock detected; the operation may be retried")]
    Deadlock,

    /// PostgreSQL error code `40001`.
    /// The transaction conflicted at the serialization level.
    /// Safe to retry with exponential back-off.
    #[error("serialization failure; the transaction may be retried")]
    SerializationFailure,

    /// The connection pool exhausted its `acquire_timeout` waiting for a free
    /// slot. Either `max_connections` is too low or a downstream stall is
    /// holding connections open.
    #[error("connection pool timed out acquiring a connection")]
    PoolTimedOut,

    /// The pool was explicitly closed before this acquire could complete.
    #[error("connection pool is closed")]
    PoolClosed,

    /// The query returned zero rows where at least one was expected.
    #[error("row not found")]
    RowNotFound,

    /// A schema migration step failed. The database may be in a partially
    /// migrated state; manual intervention is usually required.
    #[error("migration error: {message}")]
    Migration { message: String },

    /// A network, TLS, or connection-lifecycle error occurred.
    #[error("database connection error: {message}")]
    Connection { message: String },

    /// The `DATABASE_URL` or pool configuration could not be parsed.
    #[error("database configuration error: {message}")]
    Configuration { message: String },

    /// The computed [`ShardId`] has no registered pool in the cluster registry.
    ///
    /// This indicates a misconfiguration at cluster construction time — a shard
    /// that the router believes should exist has no pool backing it. This is not
    /// a transient error and cannot be retried.  `DB-8001`
    #[error("shard {shard_id} has no registered pool in the cluster")]
    ShardNotFound { shard_id: ShardId },

    /// [`TransactionManager::run`] was called against an `ApplicationSharded`
    /// topology where no shard key was provided to determine routing.
    ///
    /// Callers must migrate to [`TransactionManager::run_on_shard`].
    /// `DB-8002`
    ///
    /// [`TransactionManager::run`]: crate::transaction::manager::TransactionManager::run
    /// [`TransactionManager::run_on_shard`]: crate::transaction::manager::TransactionManager::run_on_shard
    #[error("shard routing failed: {reason}")]
    ShardRoutingFailed { reason: String },

    /// Any database error not covered by a dedicated variant.
    /// `code` is the five-character PostgreSQL SQLSTATE code.
    #[error("database error [{code}]: {message}")]
    Database { code: String, message: String },
}

impl AppError for StorageError {
    fn error_code(&self) -> &'static str {
        match self {
            StorageError::UniqueViolation { .. }     => "DB-1001",
            StorageError::ForeignKeyViolation { .. } => "DB-1002",
            StorageError::NotNullViolation { .. }    => "DB-1003",
            StorageError::CheckViolation { .. }      => "DB-1004",
            StorageError::Deadlock                   => "DB-2001",
            StorageError::SerializationFailure       => "DB-2002",
            StorageError::PoolTimedOut               => "DB-3001",
            StorageError::PoolClosed                 => "DB-3002",
            StorageError::RowNotFound                => "DB-4001",
            StorageError::Migration { .. }           => "DB-5001",
            StorageError::Connection { .. }          => "DB-6001",
            StorageError::Configuration { .. }       => "DB-7001",
            StorageError::ShardNotFound { .. }       => "DB-8001",
            StorageError::ShardRoutingFailed { .. }  => "DB-8002",
            StorageError::Database { .. }            => "DB-9000",
        }
    }

    fn http_status(&self) -> StatusCode {
        match self {
            StorageError::UniqueViolation { .. }     => StatusCode::CONFLICT,
            StorageError::ForeignKeyViolation { .. } => StatusCode::UNPROCESSABLE_ENTITY,
            StorageError::NotNullViolation { .. }    => StatusCode::UNPROCESSABLE_ENTITY,
            StorageError::CheckViolation { .. }      => StatusCode::UNPROCESSABLE_ENTITY,
            StorageError::Deadlock                   => StatusCode::SERVICE_UNAVAILABLE,
            StorageError::SerializationFailure       => StatusCode::SERVICE_UNAVAILABLE,
            StorageError::PoolTimedOut               => StatusCode::SERVICE_UNAVAILABLE,
            StorageError::PoolClosed                 => StatusCode::SERVICE_UNAVAILABLE,
            StorageError::RowNotFound                => StatusCode::NOT_FOUND,
            StorageError::Migration { .. }           => StatusCode::INTERNAL_SERVER_ERROR,
            StorageError::Connection { .. }          => StatusCode::SERVICE_UNAVAILABLE,
            StorageError::Configuration { .. }       => StatusCode::INTERNAL_SERVER_ERROR,
            StorageError::ShardNotFound { .. }       => StatusCode::SERVICE_UNAVAILABLE,
            StorageError::ShardRoutingFailed { .. }  => StatusCode::INTERNAL_SERVER_ERROR,
            StorageError::Database { .. }            => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn severity(&self) -> Severity {
        match self {
            StorageError::UniqueViolation { .. }     => Severity::Low,
            StorageError::ForeignKeyViolation { .. } => Severity::Medium,
            StorageError::NotNullViolation { .. }    => Severity::Medium,
            StorageError::CheckViolation { .. }      => Severity::Low,
            StorageError::Deadlock                   => Severity::High,
            StorageError::SerializationFailure       => Severity::High,
            StorageError::PoolTimedOut               => Severity::High,
            StorageError::PoolClosed                 => Severity::Critical,
            StorageError::RowNotFound                => Severity::Low,
            StorageError::Migration { .. }           => Severity::Critical,
            StorageError::Connection { .. }          => Severity::High,
            StorageError::Configuration { .. }       => Severity::Critical,
            StorageError::ShardNotFound { .. }       => Severity::Critical,
            StorageError::ShardRoutingFailed { .. }  => Severity::Critical,
            StorageError::Database { .. }            => Severity::Medium,
        }
    }

    fn is_retryable(&self) -> bool {
        matches!(
            self,
            StorageError::Deadlock
                | StorageError::SerializationFailure
                | StorageError::PoolTimedOut
        )
    }

    fn category(&self) -> &'static str {
        "DB"
    }

    fn user_facing_message(&self) -> &'static str {
        match self {
            StorageError::UniqueViolation { .. } =>
                "A record with this information already exists.",
            StorageError::ForeignKeyViolation { .. } =>
                "The referenced resource does not exist.",
            StorageError::NotNullViolation { .. } =>
                "A required field is missing.",
            StorageError::CheckViolation { .. } =>
                "The provided value is invalid.",
            StorageError::RowNotFound =>
                "The requested resource was not found.",
            StorageError::Deadlock | StorageError::SerializationFailure =>
                "A temporary conflict occurred. Please retry the operation.",
            StorageError::PoolTimedOut =>
                "The service is temporarily overloaded. Please retry.",
            _ =>
                "A database error occurred. Please try again later.",
        }
    }
}

impl From<sqlx::Error> for StorageError {
    fn from(err: sqlx::Error) -> Self {
        match err {
            sqlx::Error::Database(db_err) => map_database_error(db_err),
            sqlx::Error::RowNotFound      => StorageError::RowNotFound,
            sqlx::Error::PoolTimedOut     => StorageError::PoolTimedOut,
            sqlx::Error::PoolClosed       => StorageError::PoolClosed,
            sqlx::Error::WorkerCrashed    => StorageError::Connection {
                message: "database worker thread crashed unexpectedly".into(),
            },
            sqlx::Error::Io(e) => StorageError::Connection {
                message: e.to_string(),
            },
            sqlx::Error::Tls(e) => StorageError::Connection {
                message: format!("TLS error: {e}"),
            },
            sqlx::Error::Protocol(msg) => StorageError::Connection {
                message: msg,
            },
            sqlx::Error::Configuration(e) => StorageError::Configuration {
                message: e.to_string(),
            },
            sqlx::Error::Migrate(e) => StorageError::Migration {
                message: e.to_string(),
            },
            other => StorageError::Database {
                code: String::new(),
                message: other.to_string(),
            },
        }
    }
}

fn map_database_error(db_err: Box<dyn sqlx::error::DatabaseError>) -> StorageError {
    use sqlx::error::ErrorKind;

    match db_err.kind() {
        ErrorKind::UniqueViolation => StorageError::UniqueViolation {
            constraint: db_err.constraint().unwrap_or("unknown").to_owned(),
        },
        ErrorKind::ForeignKeyViolation => StorageError::ForeignKeyViolation {
            constraint: db_err.constraint().unwrap_or("unknown").to_owned(),
        },
        ErrorKind::NotNullViolation => StorageError::NotNullViolation {
            detail: db_err.message().to_owned(),
        },
        ErrorKind::CheckViolation => StorageError::CheckViolation {
            constraint: db_err.constraint().unwrap_or("unknown").to_owned(),
        },
        // Deadlock and serialization failures arrive as ErrorKind::Other;
        // discriminate by the five-character SQLSTATE code.
        _ => match db_err.code().as_deref() {
            Some("40P01") => StorageError::Deadlock,
            Some("40001") => StorageError::SerializationFailure,
            _ => StorageError::Database {
                code: db_err.code().map(|c| c.into_owned()).unwrap_or_default(),
                message: db_err.message().to_owned(),
            },
        },
    }
}
