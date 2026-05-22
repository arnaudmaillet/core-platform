// crates/shared-kernel/src/core/transaction/transaction.rs

use std::any::Any;
use std::pin::Pin;
use crate::core::Result;

pub trait Transaction: Send + Sync + Any {
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn commit(&mut self) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>>;
    fn rollback(&mut self) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>>;
}
