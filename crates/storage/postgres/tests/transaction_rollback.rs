mod common;

use common::setup::test_pool;
use postgres::{StorageError, TransactionManager};

/// Convenience alias so test closures don't need to spell out the full type.
type TestError = StorageError;

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

async fn create_rollback_table(pool: &sqlx::PgPool, table: &str) {
    sqlx::query(&format!(
        "CREATE TEMP TABLE IF NOT EXISTS {table} (
             id   SERIAL PRIMARY KEY,
             val  TEXT NOT NULL
         )"
    ))
    .execute(pool)
    .await
    .unwrap_or_else(|e| panic!("failed to create temp table '{table}': {e}"));
}

async fn row_count(pool: &sqlx::PgPool, table: &str) -> i64 {
    sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table}"))
        .fetch_one(pool)
        .await
        .unwrap_or_else(|e| panic!("count query failed on '{table}': {e}"))
}

// ─────────────────────────────────────────────────────────────────────────────
// Test: mutations inside a failing transaction must not persist
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn rollback_on_closure_error_leaves_no_rows() {
    let pool = test_pool().await;
    create_rollback_table(&pool, "rb_on_err").await;

    let mgr = TransactionManager::new(pool.clone());

    let result: Result<(), TestError> = mgr
        .run(|tx| {
            Box::pin(async move {
                sqlx::query("INSERT INTO rb_on_err (val) VALUES ('should_not_persist')")
                    .execute(&mut **tx)
                    .await
                    .map_err(StorageError::from)?;

                // Simulate an application-level failure after the INSERT.
                Err(StorageError::Database {
                    code: "TEST".into(),
                    message: "intentional failure to trigger rollback".into(),
                })
            })
        })
        .await;

    assert!(result.is_err(), "expected the transaction to propagate the error");
    assert_eq!(
        row_count(&pool, "rb_on_err").await,
        0,
        "the INSERT must have been rolled back"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Test: a successful closure must commit all mutations atomically
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn commit_on_closure_ok_persists_all_rows() {
    let pool = test_pool().await;
    create_rollback_table(&pool, "rb_commit").await;

    let mgr = TransactionManager::new(pool.clone());

    let result: Result<(), TestError> = mgr
        .run(|tx| {
            Box::pin(async move {
                for val in ["alpha", "beta", "gamma"] {
                    sqlx::query("INSERT INTO rb_commit (val) VALUES ($1)")
                        .bind(val)
                        .execute(&mut **tx)
                        .await
                        .map_err(StorageError::from)?;
                }
                Ok(())
            })
        })
        .await;

    assert!(result.is_ok(), "expected successful commit");
    assert_eq!(
        row_count(&pool, "rb_commit").await,
        3,
        "all three rows must be visible after commit"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Test: partial mutations before the failure must all be rolled back
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn rollback_is_atomic_even_after_multiple_inserts() {
    let pool = test_pool().await;
    create_rollback_table(&pool, "rb_partial").await;

    let mgr = TransactionManager::new(pool.clone());

    let result: Result<(), TestError> = mgr
        .run(|tx| {
            Box::pin(async move {
                // Two successful inserts...
                for val in ["first", "second"] {
                    sqlx::query("INSERT INTO rb_partial (val) VALUES ($1)")
                        .bind(val)
                        .execute(&mut **tx)
                        .await
                        .map_err(StorageError::from)?;
                }
                // ...followed by a simulated failure.
                Err(StorageError::Database {
                    code: "TEST".into(),
                    message: "partial failure".into(),
                })
            })
        })
        .await;

    assert!(result.is_err());
    assert_eq!(
        row_count(&pool, "rb_partial").await,
        0,
        "both prior inserts must have been rolled back atomically"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Test: TransactionManager is Clone and both instances share the same pool
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn cloned_manager_shares_pool_and_commits_independently() {
    let pool = test_pool().await;
    create_rollback_table(&pool, "rb_clone").await;

    let mgr_a = TransactionManager::new(pool.clone());
    let mgr_b = mgr_a.clone();

    // Both managers commit successfully against the same physical pool.
    let r1: Result<(), TestError> = mgr_a
        .run(|tx| {
            Box::pin(async move {
                sqlx::query("INSERT INTO rb_clone (val) VALUES ('from_a')")
                    .execute(&mut **tx)
                    .await
                    .map_err(StorageError::from)?;
                Ok(())
            })
        })
        .await;

    let r2: Result<(), TestError> = mgr_b
        .run(|tx| {
            Box::pin(async move {
                sqlx::query("INSERT INTO rb_clone (val) VALUES ('from_b')")
                    .execute(&mut **tx)
                    .await
                    .map_err(StorageError::from)?;
                Ok(())
            })
        })
        .await;

    assert!(r1.is_ok());
    assert!(r2.is_ok());
    assert_eq!(row_count(&pool, "rb_clone").await, 2);
}
