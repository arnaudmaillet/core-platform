// crates/shared-kernel/src/infrastructure/postgres/postgres_transaction_manager.rs

use std::pin::Pin;
use sqlx::{Pool, Postgres};
use crate::domain::transaction::{Transaction, TransactionManager};
use crate::infrastructure::postgres::{PostgresTransaction, SqlxErrorExt};

pub struct PostgresTransactionManager {
    pool: Pool<Postgres>,
}

impl PostgresTransactionManager {
    pub fn new(pool: Pool<Postgres>) -> Self {
        Self { pool }
    }
}

impl TransactionManager for PostgresTransactionManager {
    fn in_transaction<'a>(
        &'a self,
        f: Box<dyn FnOnce(Box<dyn Transaction>) -> Pin<Box<dyn Future<Output =crate::errors::Result<()>> + Send + 'a>> + Send + 'a>,
    ) -> Pin<Box<dyn Future<Output =crate::errors::Result<()>> + Send + 'a>> {
        let pool = self.pool.clone();
        Box::pin(async move {
            let tx = pool.begin().await.map_domain_infra("Failed to begin transaction")?;
            let sqlx_tx = Box::new(PostgresTransaction::new(tx));
            f(sqlx_tx as Box<dyn Transaction>).await
        })
    }
}