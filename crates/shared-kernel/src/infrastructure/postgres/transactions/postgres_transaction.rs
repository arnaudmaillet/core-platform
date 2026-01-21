// crates/shared-kernel/src/infrastructure/postgres/postgres_transaction.rs

use std::pin::Pin;
use sqlx::{PgConnection, PgPool, Postgres, Transaction as PostgresTx};
use crate::domain::transaction::Transaction;
use crate::errors::{Result, DomainError};

/// 1. La Structure (Le Conteneur)
pub struct PostgresTransaction {
    inner: PostgresTx<'static, Postgres>,
}

impl PostgresTransaction {
    pub fn new(tx: PostgresTx<'static, Postgres>) -> Self {
        Self { inner: tx }
    }
    pub fn get_mut(&mut self) -> &mut PostgresTx<'static, Postgres> {
        &mut self.inner
    }
}

impl Transaction for PostgresTransaction {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl dyn Transaction + '_ {
    pub async fn execute_on<'a, F, T>(
        pool: &PgPool,
        tx: Option<&'a mut dyn Transaction>,
        f: F,
    ) -> Result<T>
    where
        F: for<'b> FnOnce(&'b mut PgConnection) -> Pin<Box<dyn Future<Output = Result<T>> + Send + 'b>> + Send,
    {
        match tx {
            // Si une transaction est fournie, on fait le downcast et on l'utilise
            Some(t) => {
                let sqlx_tx = t.downcast_mut_sqlx()?;
                f(&mut **sqlx_tx).await
            }
            // Sinon, on prend une connexion simple au pool
            None => {
                let mut conn = pool.acquire().await
                    .map_err(|e| DomainError::Internal(format!("Pool acquisition failed: {}", e)))?;
                f(&mut *conn).await
            }
        }
    }
}

/// 2. Le Helper (L'outil de conversion)
pub trait TransactionExt {
    fn downcast_mut_sqlx(&mut self) -> Result<&mut PostgresTx<'static, Postgres>>;
}

impl TransactionExt for dyn Transaction + '_ {
    fn downcast_mut_sqlx(&mut self) -> Result<&mut PostgresTx<'static, Postgres>> {
        self.as_any_mut()
            .downcast_mut::<PostgresTransaction>()
            .map(|tx| tx.get_mut())
            .ok_or_else(|| DomainError::Internal("Type mismatch: Expected PostgresTransaction".into()))
    }
}
