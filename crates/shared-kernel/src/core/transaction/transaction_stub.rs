// crates/shared-kernel/src/domain/transaction/transaction_stub.rs
use std::sync::{Arc, Mutex};
use std::any::Any;
use std::pin::Pin;
use crate::core::Transaction;
use crate::core::Result;

pub struct FakeTransaction {
    pub committed: Arc<Mutex<bool>>,
    pub rolled_back: Arc<Mutex<bool>>,
}

impl FakeTransaction {
    pub fn new() -> Self {
        Self {
            committed: Arc::new(Mutex::new(false)),
            rolled_back: Arc::new(Mutex::new(false)),
        }
    }
}

impl Transaction for FakeTransaction {
    fn as_any_mut(&mut self) -> &mut dyn Any { self }

    fn commit(&mut self) -> Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + '_>> {
        let committed = self.committed.clone();
        Box::pin(async move {
            let mut lock = committed.lock().unwrap();
            *lock = true;
            Ok(())
        })
    }

    fn rollback(&mut self) -> Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + '_>> {
        let rolled_back = self.rolled_back.clone();
        Box::pin(async move {
            let mut lock = rolled_back.lock().unwrap();
            *lock = true;
            Ok(())
        })
    }
}