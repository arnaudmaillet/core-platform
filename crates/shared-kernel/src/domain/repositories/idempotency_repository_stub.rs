// crates/shared_kernel/src/domain/repositories/idempotency_repository_stub.rs

use crate::domain::repositories::IdempotencyRepository;
use crate::domain::transaction::Transaction;
use crate::errors::{DomainError, Result};
use async_trait::async_trait;
use std::sync::Mutex;
use uuid::Uuid;
use std::collections::HashSet;

#[derive(Default)]
pub struct IdempotencyRepositoryStub {
    // On utilise un HashSet pour simuler la table unique en DB
    processed_ids: Mutex<HashSet<Uuid>>,
    error_to_return: Mutex<Option<DomainError>>,
}

impl IdempotencyRepositoryStub {
    pub fn new() -> Self {
        Self::default()
    }

    /// Simule le fait qu'une commande a déjà été traitée (pour tester le rejet)
    pub fn seed(&self, command_id: Uuid) {
        let mut ids = self.processed_ids.lock().expect("Lock failed");
        ids.insert(command_id);
    }

    pub fn set_error(&self, err: DomainError) {
        let mut slot = self.error_to_return.lock().expect("Lock failed");
        *slot = Some(err);
    }

    fn check_error(&self) -> Result<()> {
        if let Some(err) = self.error_to_return.lock().expect("Lock failed").clone() {
            return Err(err);
        }
        Ok(())
    }
}

#[async_trait]
impl IdempotencyRepository for IdempotencyRepositoryStub {
    async fn exists(&self, _tx: &mut dyn Transaction, command_id: &Uuid) -> Result<bool> {
        self.check_error()?;
        let ids = self.processed_ids.lock().expect("Lock failed");
        Ok(ids.contains(command_id))
    }

    async fn save(&self, _tx: &mut dyn Transaction, command_id: &Uuid) -> Result<()> {
        self.check_error()?;
        let mut ids = self.processed_ids.lock().expect("Lock failed");
        ids.insert(*command_id);
        Ok(())
    }
}