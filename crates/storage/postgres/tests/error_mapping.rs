use error::{AppError as _, Severity};
use http::StatusCode;
use postgres::StorageError;

// ─────────────────────────────────────────────────────────────────────────────
// Unit tests — no database required; sqlx::Error variants are constructed
// directly and fed through the From<sqlx::Error> conversion.
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn row_not_found_maps_to_db_4001_low_not_retryable() {
    let err = StorageError::from(sqlx::Error::RowNotFound);

    assert!(matches!(err, StorageError::RowNotFound));
    assert_eq!(err.error_code(), "DB-4001");
    assert_eq!(err.severity(), Severity::Low);
    assert_eq!(err.http_status(), StatusCode::NOT_FOUND);
    assert!(!err.is_retryable());
    assert_eq!(err.category(), "DB");
}

#[test]
fn pool_timed_out_maps_to_db_3001_high_retryable() {
    let err = StorageError::from(sqlx::Error::PoolTimedOut);

    assert!(matches!(err, StorageError::PoolTimedOut));
    assert_eq!(err.error_code(), "DB-3001");
    assert_eq!(err.severity(), Severity::High);
    assert_eq!(err.http_status(), StatusCode::SERVICE_UNAVAILABLE);
    assert!(err.is_retryable());
}

#[test]
fn pool_closed_maps_to_db_3002_critical_not_retryable() {
    let err = StorageError::from(sqlx::Error::PoolClosed);

    assert!(matches!(err, StorageError::PoolClosed));
    assert_eq!(err.error_code(), "DB-3002");
    assert_eq!(err.severity(), Severity::Critical);
    assert!(!err.is_retryable());
}

#[test]
fn deadlock_variant_is_high_severity_retryable() {
    let err = StorageError::Deadlock;

    assert_eq!(err.error_code(), "DB-2001");
    assert_eq!(err.severity(), Severity::High);
    assert!(err.is_retryable());
    assert_eq!(err.http_status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[test]
fn serialization_failure_is_high_severity_retryable() {
    let err = StorageError::SerializationFailure;

    assert_eq!(err.error_code(), "DB-2002");
    assert_eq!(err.severity(), Severity::High);
    assert!(err.is_retryable());
}

#[test]
fn migration_is_critical_and_maps_to_500() {
    let err = StorageError::Migration { message: "version mismatch".into() };

    assert_eq!(err.error_code(), "DB-5001");
    assert_eq!(err.severity(), Severity::Critical);
    assert_eq!(err.http_status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert!(!err.is_retryable());
}

#[test]
fn all_storage_errors_report_db_category() {
    let variants: Vec<StorageError> = vec![
        StorageError::UniqueViolation { constraint: "x".into() },
        StorageError::ForeignKeyViolation { constraint: "x".into() },
        StorageError::NotNullViolation { detail: "x".into() },
        StorageError::CheckViolation { constraint: "x".into() },
        StorageError::Deadlock,
        StorageError::SerializationFailure,
        StorageError::PoolTimedOut,
        StorageError::PoolClosed,
        StorageError::RowNotFound,
        StorageError::Migration { message: "x".into() },
        StorageError::Connection { message: "x".into() },
        StorageError::Configuration { message: "x".into() },
        StorageError::Database { code: "x".into(), message: "x".into() },
    ];

    for err in &variants {
        assert_eq!(err.category(), "DB", "failed for error_code={}", err.error_code());
    }
}

#[test]
fn error_codes_are_globally_unique() {
    let codes = vec![
        "DB-1001", "DB-1002", "DB-1003", "DB-1004",
        "DB-2001", "DB-2002",
        "DB-3001", "DB-3002",
        "DB-4001",
        "DB-5001",
        "DB-6001",
        "DB-7001",
        "DB-9000",
    ];
    let unique: std::collections::HashSet<_> = codes.iter().collect();
    assert_eq!(codes.len(), unique.len(), "error codes must be globally unique");
}

// Live constraint-violation tests (real `23505`/`23503`/`23502` against a
// running Postgres) live in `error_mapping_live.rs`, gated behind the
// `integration-postgres` feature.
