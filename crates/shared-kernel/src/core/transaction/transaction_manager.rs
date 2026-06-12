// crates/shared-kernel/src/domain/transaction/transaction_manager.rs

use crate::core::{Result, Transaction};
use std::pin::Pin;

pub trait TransactionManager: Send + Sync {
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
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>>;
}

pub trait TransactionManagerExt: TransactionManager {
    fn run_transaction<'a, F>(
        &'a self,
        f: F,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>>
    where
        F: FnOnce(&'a mut dyn Transaction) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>>
            + Send
            + 'a,
    {
        let boxed_closure = Box::new(move |tx: &mut dyn Transaction| {
            let unsafe_tx =
                unsafe { std::mem::transmute::<&mut dyn Transaction, &'a mut dyn Transaction>(tx) };
            f(unsafe_tx)
        });

        TransactionManager::run_in_transaction(self, boxed_closure)
    }
}

impl<T: TransactionManager + ?Sized> TransactionManagerExt for T {}
