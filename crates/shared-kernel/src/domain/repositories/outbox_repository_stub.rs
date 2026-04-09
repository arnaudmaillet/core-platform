// crates/shared-kernel/src/domain/repositories/outbox_repository_stub.rs

use std::sync::{Arc, Mutex};
use async_trait::async_trait;
use crate::domain::events::{DomainEvent, EventEnvelope};
use crate::domain::repositories::OutboxRepository;
use crate::domain::transaction::Transaction;
use crate::errors::{DomainError, Result};

pub struct OutboxRepositoryStub {
    /// Liste des types d'événements sauvegardés pour vérification
    pub saved_events: Arc<Mutex<Vec<String>>>,
    /// Permet de forcer une erreur pour tester le rollback
    pub force_error: Arc<Mutex<Option<DomainError>>>,
}

impl OutboxRepositoryStub {
    pub fn new() -> Self {
        Self {
            saved_events: Arc::new(Mutex::new(vec![])),
            force_error: Arc::new(Mutex::new(None)),
        }
    }
}

#[async_trait]
impl OutboxRepository for OutboxRepositoryStub {
    async fn save_all(&self, _tx: &mut dyn Transaction, events: &[&dyn DomainEvent]) -> Result<()> {
        if let Some(err) = self.force_error.lock().unwrap().clone() {
            return Err(err);
        }

        if events.is_empty() {
            return Ok(());
        }

        let mut saved = self.saved_events.lock().unwrap();
        for event in events {
            saved.push(event.event_type().to_string());
        }
        
        Ok(())
    }

    async fn find_pending(&self, _limit: i32) -> Result<Vec<EventEnvelope>> {
        Ok(vec![])
    }
}