//! Live error-mapping suite — triggers real Postgres constraint violations and
//! verifies the `From<sqlx::Error>` mapping chain end-to-end.
//!
//! Requires a reachable PostgreSQL instance via `DATABASE_URL`. Opt-in via
//! `cargo test -p postgres --features integration-postgres`, matching the
//! fleet's `integration-<crate>` convention.
#![cfg(feature = "integration-postgres")]

mod common;

use common::setup::test_pool;
use error::{AppError as _, Severity};
use http::StatusCode;
use postgres::StorageError;

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
