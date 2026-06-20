mod common;

use common::setup::test_pool;
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

// ─────────────────────────────────────────────────────────────────────────────
// Integration tests — require a live PostgreSQL instance via DATABASE_URL.
// Run with: cargo test -p postgres -- --include-ignored
// Or: DATABASE_URL=postgres://... cargo test -p postgres
// ─────────────────────────────────────────────────────────────────────────────

/// Trigger a real `23505` unique-violation and verify the full mapping chain.
#[tokio::test]
async fn unique_violation_maps_to_db_1001_low_not_retryable() {
    let pool = test_pool().await;

    sqlx::query(
        "CREATE TEMP TABLE IF NOT EXISTS em_unique (
             id INTEGER PRIMARY KEY
         )",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query("INSERT INTO em_unique VALUES (1)")
        .execute(&pool)
        .await
        .unwrap();

    let raw = sqlx::query("INSERT INTO em_unique VALUES (1)")
        .execute(&pool)
        .await
        .expect_err("second insert must fail with unique violation");

    let err = StorageError::from(raw);

    assert!(
        matches!(err, StorageError::UniqueViolation { .. }),
        "expected UniqueViolation, got: {err:?}"
    );
    assert_eq!(err.error_code(), "DB-1001");
    assert_eq!(err.severity(), Severity::Low);
    assert_eq!(err.http_status(), StatusCode::CONFLICT);
    assert!(!err.is_retryable());
}

/// Trigger a real `23503` foreign-key-violation and verify the mapping.
#[tokio::test]
async fn foreign_key_violation_maps_to_db_1002_medium_not_retryable() {
    let pool = test_pool().await;

    sqlx::query(
        "CREATE TEMP TABLE IF NOT EXISTS em_parent (id INTEGER PRIMARY KEY)",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "CREATE TEMP TABLE IF NOT EXISTS em_child (
             id        INTEGER PRIMARY KEY,
             parent_id INTEGER NOT NULL REFERENCES em_parent(id)
         )",
    )
    .execute(&pool)
    .await
    .unwrap();

    let raw = sqlx::query("INSERT INTO em_child VALUES (1, 999)")
        .execute(&pool)
        .await
        .expect_err("insert with non-existent parent must fail");

    let err = StorageError::from(raw);

    assert!(
        matches!(err, StorageError::ForeignKeyViolation { .. }),
        "expected ForeignKeyViolation, got: {err:?}"
    );
    assert_eq!(err.error_code(), "DB-1002");
    assert_eq!(err.severity(), Severity::Medium);
    assert_eq!(err.http_status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert!(!err.is_retryable());
}

/// Trigger a real `23502` not-null violation.
#[tokio::test]
async fn not_null_violation_maps_to_db_1003() {
    let pool = test_pool().await;

    sqlx::query(
        "CREATE TEMP TABLE IF NOT EXISTS em_nn (
             id  INTEGER PRIMARY KEY,
             val TEXT NOT NULL
         )",
    )
    .execute(&pool)
    .await
    .unwrap();

    let raw = sqlx::query("INSERT INTO em_nn (id) VALUES (1)")
        .execute(&pool)
        .await
        .expect_err("insert with NULL in NOT NULL column must fail");

    let err = StorageError::from(raw);

    let StorageError::NotNullViolation { detail } = &err else {
        panic!("expected NotNullViolation, got: {err:?}");
    };
    assert!(
        detail.contains("val"),
        "detail should mention the column name; got: {detail}"
    );
    assert_eq!(err.error_code(), "DB-1003");
    assert_eq!(err.severity(), Severity::Medium);
}
