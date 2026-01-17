// crates/shared-kernel/src/infrastructure/transaction.rs

use std::any::Any;
use std::future::Future;
use std::pin::Pin;

use sqlx::{PgConnection, PgPool};
use crate::domain::transaction::Transaction;
use crate::infrastructure::postgres::PostgresTransaction;

use crate::errors::{Result, DomainError};

// L'Extension Trait pour ajouter la méthode helper sur le trait object
pub trait TransactionExt {
    fn downcast_mut_sqlx(&mut self) -> Result<&mut sqlx::Transaction<'static, sqlx::Postgres>>;
}

// 3. Implémentation de l'extension pour TOUT ce qui ressemble à une Transaction
impl TransactionExt for dyn Transaction + '_ {
    fn downcast_mut_sqlx(&mut self) -> Result<&mut sqlx::Transaction<'static, sqlx::Postgres>> {
        self.as_any_mut()
            .downcast_mut::<PostgresTransaction>() // Ton wrapper concret
            .map(|tx| tx.get_mut())
            .ok_or_else(|| DomainError::Internal("Transaction type mismatch: Expected SqlxTx".into()))
    }
}

// Pour supporter aussi les &mut dyn Transaction
impl TransactionExt for &mut (dyn Transaction + '_) {
    fn downcast_mut_sqlx(&mut self) -> Result<&mut sqlx::Transaction<'static, sqlx::Postgres>> {
        (**self).downcast_mut_sqlx()
    }
}

impl dyn Transaction + '_ {
    pub async fn execute_on<F, T>(
        pool: &PgPool,
        tx: Option<&mut dyn Transaction>,
        f: F,
    ) -> Result<T>
    where
        F: for<'a> FnOnce(&'a mut PgConnection) -> Pin<Box<dyn Future<Output = Result<T>> + Send + 'a>> + Send,
    {
        match tx {
            Some(t) => {
                let sqlx_tx = t.downcast_mut_sqlx()?;
                f(&mut **sqlx_tx).await
            }
            None => {
                let mut conn = pool.acquire().await
                    .map_err(|e| DomainError::Internal(e.to_string()))?;
                f(&mut *conn).await
            }
        }
    }
}