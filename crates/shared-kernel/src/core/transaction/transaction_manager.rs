// crates/shared-kernel/src/domain/transaction/transaction_manager.rs

use crate::core::{Result, Transaction};
use std::future::Future;
use std::pin::Pin;

/// Le futur est désormais générique sur T
pub type TransactionFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T>> + Send + 'a>>;

pub trait TransactionManager: Send + Sync {
    fn in_transaction<'a, T, F>(&'a self, f: F) -> TransactionFuture<'a, T>
    where
        F: FnOnce(Box<dyn Transaction>) -> TransactionFuture<'a, T> + Send + 'a,
        T: Send + 'a;
}
