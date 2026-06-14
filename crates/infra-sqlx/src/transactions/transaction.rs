// crates/infra-sqlx/src/transactions/transaction.rs

use shared_kernel::core::{Error, Result, Transaction};
use sqlx::{PgConnection, PgPool, Postgres, Transaction as PostgresTx};
use std::future::Future;
use std::pin::Pin;

pub struct PostgresTransaction {
    inner: Option<PostgresTx<'static, Postgres>>,
}

impl PostgresTransaction {
    pub fn new(tx: PostgresTx<'static, Postgres>) -> Self {
        Self { inner: Some(tx) }
    }
    pub fn get_mut(&mut self) -> &mut PostgresTx<'static, Postgres> {
        self.inner.as_mut().expect("Transaction already consumed")
    }

    pub fn into_inner(mut self) -> PostgresTx<'static, Postgres> {
        self.inner.take().expect("Transaction already consumed")
    }
}

impl Transaction for PostgresTransaction {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn commit(&mut self) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        let tx = self.inner.take();
        Box::pin(async move {
            if let Some(t) = tx {
                t.commit()
                    .await
                    .map_err(|e| Error::internal(format!("Commit failed: {}", e)))?;
            }
            Ok(())
        })
    }

    fn rollback(&mut self) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        let tx = self.inner.take();
        Box::pin(async move {
            if let Some(t) = tx {
                t.rollback()
                    .await
                    .map_err(|e| Error::internal(format!("Rollback failed: {}", e)))?;
            }
            Ok(())
        })
    }
}

pub trait TransactionExt {
    fn downcast_mut_sqlx(&mut self) -> Result<&mut PostgresTx<'static, Postgres>>;
}

impl TransactionExt for dyn Transaction + '_ {
    fn downcast_mut_sqlx(&mut self) -> Result<&mut PostgresTx<'static, Postgres>> {
        self.as_any_mut()
            .downcast_mut::<PostgresTransaction>()
            .map(|tx| tx.get_mut())
            .ok_or_else(|| Error::internal("Type mismatch: Expected PostgresTransaction"))
    }
}

pub trait TransactionExecuteExt {
    fn execute_on<'a, F, T>(
        &self,
        tx: Option<&'a mut (dyn Transaction + '_)>,
        f: F,
    ) -> Pin<Box<dyn Future<Output = Result<T>> + Send + 'a>>
    where
        F: for<'b> FnOnce(
                &'b mut PgConnection,
            ) -> Pin<Box<dyn Future<Output = Result<T>> + Send + 'b>>
            + Send
            + 'a;
}

impl TransactionExecuteExt for PgPool {
    fn execute_on<'a, F, T>(
        &self,
        mut tx: Option<&'a mut (dyn Transaction + '_)>,
        f: F,
    ) -> Pin<Box<dyn Future<Output = Result<T>> + Send + 'a>>
    where
        F: for<'b> FnOnce(
                &'b mut PgConnection,
            ) -> Pin<Box<dyn Future<Output = Result<T>> + Send + 'b>>
            + Send
            + 'a,
    {
        let pool = self.clone();
        Box::pin(async move {
            match tx.as_deref_mut() {
                Some(t) => {
                    let sqlx_tx = t.downcast_mut_sqlx()?;
                    f(sqlx_tx).await
                }
                None => {
                    let mut conn = pool
                        .acquire()
                        .await
                        .map_err(|e| Error::internal(format!("Pool acquisition failed: {}", e)))?;
                    f(&mut conn).await
                }
            }
        })
    }
}
