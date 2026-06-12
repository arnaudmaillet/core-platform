// crates/shared-kernel/src/domain/transaction/transaction_manager_stub.rs

use crate::repositories::TransactionStub;
use shared_kernel::core::{Result, Transaction, TransactionManager};
use std::future::Future;
use std::pin::Pin;

#[derive(Clone)]
pub struct TransactionManagerStub;

impl TransactionManager for TransactionManagerStub {
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
        Box::pin(async move {
            let mut tx = TransactionStub::new();

            f(&mut tx).await?;

            Ok(())
        })
    }
}
