use std::any::Any;
use std::pin::Pin;
use crate::domain::transaction::Transaction;

// --- TRANSACTION MANAGEMENT ---
pub struct FakeTransaction;

impl Transaction for FakeTransaction {
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    // Indispensable pour la "dyn compatibility" du trait
    fn commit(&mut self) -> Pin<Box<dyn std::future::Future<Output =crate::errors::Result<()>> + Send + '_>> {
        Box::pin(async {
            println!("🛠️ FakeTransaction: commit called");
            Ok(())
        })
    }
}