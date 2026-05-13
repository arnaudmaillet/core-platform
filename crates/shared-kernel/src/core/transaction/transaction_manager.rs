// crates/shared-kernel/src/domain/transaction/transaction_manager.rs

use crate::core::{Result, Transaction};
use std::future::Future;
use std::pin::Pin;

/// Alias pour le futur retourné par la transaction
pub type TransactionFuture<'a, T = ()> = Pin<Box<dyn Future<Output = Result<T>> + Send + 'a>>;

/// Alias pour la closure de transaction (le "travail" à accomplir)
pub type TransactionWork<'a> =
    Box<dyn FnOnce(Box<dyn Transaction>) -> TransactionFuture<'a> + Send + 'a>;

pub trait TransactionManager: Send + Sync {
    fn in_transaction<'a>(&'a self, f: TransactionWork<'a>) -> TransactionFuture<'a>;
}
