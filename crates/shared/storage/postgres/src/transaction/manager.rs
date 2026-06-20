use crate::error::StorageError;
use futures::future::BoxFuture;
use sqlx::PgPool;

/// Owned, pool-scoped PostgreSQL transaction handle.
///
/// `'static` reflects that `pool.begin()` moves an owned connection out of the
/// pool rather than borrowing one; the transaction owns that connection for its
/// entire lifetime.
pub type PgTransaction = sqlx::Transaction<'static, sqlx::Postgres>;

/// Thin, clone-friendly wrapper around a [`PgPool`] that provides a single,
/// safe entry point for executing ACID-compliant operations.
///
/// [`TransactionManager`] is intentionally domain-agnostic: it knows nothing
/// about tables, entities, or application patterns (outbox, idempotency, etc.).
/// Those concerns belong in the CQRS command handlers that call [`Self::run`].
///
/// # Clone semantics
///
/// [`PgPool`] is an `Arc<PgPoolInner>` internally, so cloning is `O(1)` and
/// both instances share the same underlying pool.
#[derive(Clone, Debug)]
pub struct TransactionManager {
    pool: PgPool,
}

impl TransactionManager {
    /// Creates a new manager backed by `pool`.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Returns a reference to the inner pool for callers that need raw access
    /// (e.g., running non-transactional reads or issuing `COPY` commands).
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Executes `f` inside a single database transaction with full ACID
    /// guarantees.
    ///
    /// **Commit path:** if `f` returns `Ok(T)`, the transaction is committed
    /// before this function returns.
    ///
    /// **Rollback path:** if `f` returns `Err(E)`, the transaction is rolled
    /// back (best-effort; sqlx's `Drop` impl on [`PgTransaction`] provides a
    /// safety-net rollback if the explicit call were to panic).
    ///
    /// # Type parameters
    ///
    /// - `T` — the success value produced by `f`.
    /// - `E` — the caller's error type. Must implement `From<StorageError>` so
    ///   that pool-acquire and commit failures can be surfaced without an
    ///   additional mapping layer. A CQRS command handler achieving this by
    ///   adding `#[from] StorageError` to its error enum.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let result = tx_mgr
    ///     .run(|tx| Box::pin(async move {
    ///         sqlx::query("INSERT INTO accounts (id) VALUES ($1)")
    ///             .bind(account_id)
    ///             .execute(&mut **tx)
    ///             .await
    ///             .map_err(CommandError::from)?;
    ///
    ///         sqlx::query("INSERT INTO ledger (account_id, amount) VALUES ($1, $2)")
    ///             .bind(account_id)
    ///             .bind(initial_balance)
    ///             .execute(&mut **tx)
    ///             .await
    ///             .map_err(CommandError::from)?;
    ///
    ///         Ok(())
    ///     }))
    ///     .await?;
    /// ```
    #[tracing::instrument(name = "db.transaction", skip(self, f))]
    pub async fn run<F, T, E>(&self, f: F) -> Result<T, E>
    where
        F: for<'tx> FnOnce(&'tx mut PgTransaction) -> BoxFuture<'tx, Result<T, E>>,
        E: From<StorageError> + Send,
        T: Send,
    {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| E::from(StorageError::from(e)))?;

        match f(&mut tx).await {
            Ok(value) => {
                tx.commit()
                    .await
                    .map_err(|e| E::from(StorageError::from(e)))?;
                Ok(value)
            }
            Err(e) => {
                // Best-effort; `Drop` on `Transaction` issues a rollback if
                // this explicit call fails or is skipped.
                let _ = tx.rollback().await;
                Err(e)
            }
        }
    }
}
