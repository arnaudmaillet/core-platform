// crates/shared_kernel/src/domain/repositories/idempotency_repository_stub.rs

use crate::core::{Error, Result, Transaction};
use crate::idempotency::IdempotencyRepository;
use async_trait::async_trait;
use std::collections::HashSet;
use std::sync::Mutex;
use uuid::Uuid;

#[derive(Default)]
pub struct IdempotencyRepositoryStub {
    processed_ids: Mutex<HashSet<Uuid>>,
    error_to_return: Mutex<Option<Error>>,
}

impl IdempotencyRepositoryStub {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn seed(&self, command_id: Uuid) {
        let mut ids = self.processed_ids.lock().expect("Lock failed");
        ids.insert(command_id);
    }

    pub fn set_error(&self, err: Error) {
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
    async fn exists(
        &self,
        _tx: Option<&mut (dyn Transaction + '_)>,
        command_id: &Uuid,
    ) -> Result<bool> {
        self.check_error()?;
        let ids = self.processed_ids.lock().expect("Lock failed");
        Ok(ids.contains(command_id))
    }

    async fn save(
        &self,
        _tx: Option<&mut (dyn Transaction + '_)>,
        command_id: &Uuid,
    ) -> Result<()> {
        self.check_error()?;
        let mut ids = self.processed_ids.lock().expect("Lock failed");
        ids.insert(*command_id);
        Ok(())
    }
}
