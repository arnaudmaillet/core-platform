// crates/shared-kernel/src/domain/transaction/transaction.rs

use std::any::Any;
use std::pin::Pin;
use crate::errors::Result;

pub trait Transaction: Send + Sync + Any {
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn commit(&mut self) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>>;
}
