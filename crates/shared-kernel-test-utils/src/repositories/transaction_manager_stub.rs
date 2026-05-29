// crates/shared-kernel/src/domain/transaction/transaction_manager_stub.rs

use crate::repositories::TransactionStub;
use shared_kernel::core::{Result, Transaction, TransactionManager};
use std::future::Future;
use std::pin::Pin;

#[derive(Clone)]
pub struct TransactionManagerStub;

impl TransactionManager for TransactionManagerStub {
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
        Box::pin(async move {
            let tx = Box::new(TransactionStub::new());
            f(tx as Box<dyn Transaction>).await
        })
    }
}
