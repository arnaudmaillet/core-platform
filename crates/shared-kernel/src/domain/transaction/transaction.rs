// crates/shared-kernel/src/domain/transaction/transaction.rs

use std::any::Any;

pub trait Transaction: Send + Sync + Any {
    fn as_any_mut(&mut self) -> &mut dyn Any;
}