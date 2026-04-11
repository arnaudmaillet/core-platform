// crates/shared_kernel/src/domain/repositories/outbox_repository_stub.rs

use crate::domain::events::{DomainEvent, EventEnvelope};
use crate::domain::repositories::OutboxRepository;
use crate::domain::transaction::Transaction;
use crate::errors::{DomainError, Result};
use async_trait::async_trait;
use std::sync::{Arc, Mutex};

pub struct OutboxRepositoryStub {
    saved_events: Arc<Mutex<Vec<String>>>,
    error_to_return: Arc<Mutex<Option<DomainError>>>,
}

impl OutboxRepositoryStub {
    pub fn new() -> Self {
        Self {
            saved_events: Arc::new(Mutex::new(vec![])),
            error_to_return: Arc::new(Mutex::new(None)),
        }
    }

    // --- Helpers pour l'Arrange ---

    /// Force une erreur lors du prochain save_all (ex: simulate Kafka/DB failure)
    pub fn set_error(&self, err: DomainError) {
        let mut slot = self.error_to_return.lock().expect("Lock failed");
        *slot = Some(err);
    }

    /// Réinitialise les événements capturés
    pub fn clear(&self) {
        self.saved_events.lock().expect("Lock failed").clear();
        *self.error_to_return.lock().expect("Lock failed") = None;
    }

    // --- Helpers pour l'Assert ---

    /// Retourne le nombre d'événements capturés
    pub fn count(&self) -> usize {
        self.saved_events.lock().expect("Lock failed").len()
    }

    /// Retourne la liste des noms d'événements (ex: "AccountBanned")
    pub fn event_names(&self) -> Vec<String> {
        self.saved_events.lock().expect("Lock failed").clone()
    }

    // --- Logique interne ---

    fn check_error(&self) -> Result<()> {
        if let Some(err) = self.error_to_return.lock().expect("Lock failed").clone() {
            return Err(err);
        }
        Ok(())
    }
}

#[async_trait]
impl OutboxRepository for OutboxRepositoryStub {
    async fn save_all(&self, _tx: &mut dyn Transaction, events: &[&dyn DomainEvent]) -> Result<()> {
        self.check_error()?;

        if events.is_empty() {
            return Ok(());
        }

        let mut saved = self.saved_events.lock().expect("Lock failed");
        for event in events {
            // On stocke le nom du type d'événement pour faciliter les tests
            saved.push(event.event_name().to_string());
        }

        Ok(())
    }

    async fn find_pending(&self, _limit: i32) -> Result<Vec<EventEnvelope>> {
        self.check_error()?;
        Ok(vec![])
    }
}
