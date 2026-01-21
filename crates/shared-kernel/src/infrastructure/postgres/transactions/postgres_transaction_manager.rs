// crates/shared-kernel/src/infrastructure/postgres/postgres_transaction_manager.rs

use std::pin::Pin;
use std::future::Future;
use sqlx::{Pool, Postgres};
use crate::domain::transaction::{Transaction, TransactionManager};
use crate::infrastructure::postgres::transactions::PostgresTransaction;
use crate::infrastructure::postgres::mappers::SqlxErrorExt;
use crate::errors::Result;

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
        f: Box<dyn FnOnce(Box<dyn Transaction>) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> + Send + 'a>,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
        let pool = self.pool.clone();
        Box::pin(async move {
            let tx = pool.begin().await.map_domain_infra("Failed to begin transaction")?;
            let sqlx_tx = Box::new(PostgresTransaction::new(tx));
            f(sqlx_tx as Box<dyn Transaction>).await
        })
    }
}

// Optionnel : Ajoute ici le helper générique si tu veux éviter un fichier de plus
pub trait TransactionManagerExt: TransactionManager {
    fn run_in_transaction<'a, F, Fut>(&'a self, f: F) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>>
    where
        F: FnOnce(Box<dyn Transaction>) -> Fut + Send + 'a,
        Fut: Future<Output = Result<()>> + Send + 'a,
    {
        self.in_transaction(Box::new(move |tx| Box::pin(f(tx))))
    }
}
impl<T: TransactionManager + ?Sized> TransactionManagerExt for T {}