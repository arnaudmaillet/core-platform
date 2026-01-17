// crates/shared-kernel/src/domain/transaction/transaction_manager.rs

use std::future::Future;
use std::pin::Pin;
use crate::domain::transaction::Transaction;
use crate::errors::Result;

pub trait TransactionManager: Send + Sync {
    fn in_transaction<'a>(
        &'a self,
        f: Box<dyn FnOnce(Box<dyn Transaction>) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> + Send + 'a>,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>>;
}