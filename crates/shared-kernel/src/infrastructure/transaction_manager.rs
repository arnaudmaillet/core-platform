// crates/shared-kernel/src/persistence/transaction_manager.rs

use std::pin::Pin;
use crate::domain::transaction::{Transaction, TransactionManager};
use crate::errors::Result;

pub trait TransactionManagerExt: TransactionManager {
    fn run_in_transaction<'a, F, Fut>(&'a self, f: F) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>>
    where
        F: FnOnce(Box<dyn Transaction>) -> Fut + Send + 'a,
        Fut: Future<Output = Result<()>> + Send + 'a,
    {
        self.in_transaction(Box::new(move |tx| {
            Box::pin(f(tx))
        }))
    }
}

impl<T: TransactionManager + ?Sized> TransactionManagerExt for T {}