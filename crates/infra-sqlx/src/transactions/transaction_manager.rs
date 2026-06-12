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
    fn run_in_transaction<'a>(
        &'a self,
        f: Box<
            dyn for<'b> FnOnce(
                    &'b mut dyn Transaction,
                )
                    -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>>
                + Send
                + 'a,
        >,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
        let pool = self.pool.clone();

        Box::pin(async move {
            let tx = pool
                .begin()
                .await
                .map_err(|_| Error::database("Failed to begin transaction"))?;

            let mut sqlx_tx = PostgresTransaction::new(tx);

            match f(&mut sqlx_tx).await {
                Ok(()) => {
                    sqlx_tx.commit().await?;
                    Ok(())
                }
                Err(e) => {
                    let _ = sqlx_tx.rollback().await;
                    Err(e)
                }
            }
        })
    }
}
