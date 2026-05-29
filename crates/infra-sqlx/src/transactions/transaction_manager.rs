// crates/infra-sqlx/src/transactions/transaction_manager.rs

use crate::PostgresTransaction;
use shared_kernel::core::{Error, Result, Transaction, TransactionManager};
use sqlx::{Pool, Postgres};
use std::future::Future;
use std::pin::Pin;

#[derive(Clone)]
pub struct PostgresTransactionManager {
    pool: Pool<Postgres>,
}

impl PostgresTransactionManager {
    pub fn new(pool: Pool<Postgres>) -> Self {
        Self { pool }
    }
}

impl TransactionManager for PostgresTransactionManager {
    fn in_transaction<'a, T, F>(
        &'a self,
        f: F,
    ) -> Pin<Box<dyn Future<Output = Result<T>> + Send + 'a>>
    where
        F: FnOnce(Box<dyn Transaction>) -> Pin<Box<dyn Future<Output = Result<T>> + Send + 'a>>
            + Send
            + 'a,
        T: Send + 'a,
    {
        let pool = self.pool.clone();
        Box::pin(async move {
            let tx = pool
                .begin()
                .await
                .map_err(|_| Error::database("Failed to begin transaction"))?;
            let sqlx_tx = Box::new(PostgresTransaction::new(tx));

            f(sqlx_tx as Box<dyn Transaction>).await
        })
    }
}

pub trait TransactionManagerExt: TransactionManager {
    fn run_in_transaction<'a, F, Fut, T>(
        &'a self,
        f: F,
    ) -> Pin<Box<dyn Future<Output = Result<T>> + Send + 'a>>
    where
        F: FnOnce(Box<dyn Transaction>) -> Fut + Send + 'a,
        Fut: Future<Output = Result<T>> + Send + 'a,
        T: Send + 'a,
    {
        self.in_transaction(Box::new(move |tx| {
            let boxed_future: Pin<Box<dyn Future<Output = Result<T>> + Send + 'a>> =
                Box::pin(f(tx));
            boxed_future
        }))
    }
}
impl<T: TransactionManager + ?Sized> TransactionManagerExt for T {}
