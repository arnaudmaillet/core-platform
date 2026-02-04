// crates/shared-kernel/src/application/workers/outbox_processor.rs

use crate::application::ports::MessageProducer;
use crate::domain::repositories::OutboxStore;
use crate::errors::AppResult;
use std::time::Duration;
use tokio::time::sleep;

pub struct OutboxProcessor<Store, Broker>
where
    Store: OutboxStore,
    Broker: MessageProducer,
{
    store: Store,
    broker: Broker,
    batch_size: u32,
    polling_interval: Duration,
}

impl<Store, Broker> OutboxProcessor<Store, Broker>
where
    Store: OutboxStore,
    Broker: MessageProducer,
{
    pub fn new(store: Store, broker: Broker, batch_size: u32, interval: Duration) -> Self {
        Self {
            store,
            broker,
            batch_size,
            polling_interval: interval,
        }
    }

    pub async fn run(&self, mut shutdown_signal: tokio::sync::watch::Receiver<bool>) {
        tracing::info!("Outbox processor started");

        loop {
            // 1. Vérification immédiate du signal d'arrêt
            if *shutdown_signal.borrow() {
                break;
            }

            // 2. Traitement d'un batch
            let result = self.process_batch().await;

            let mut processed_count = 0;
            match result {
                Ok(count) => {
                    processed_count = count;
                    if count > 0 {
                        tracing::info!("Relayed {} events", count);
                    }
                }
                Err(e) => {
                    tracing::error!("Relay error: {:?}", e);
                }
            }

            // 3. Logique d'attente intelligente
            // Si on a traité un batch COMPLET, on reboucle vite (pour vider le backlog)
            // Sinon (erreur ou file vide), on attend le prochain intervalle ou le signal d'arrêt
            if processed_count < self.batch_size as usize {
                tokio::select! {
                    _ = sleep(self.polling_interval) => {},
                    _ = shutdown_signal.changed() => break,
                }
            }
        }

        tracing::info!("Outbox processor stopped gracefully");
    }

    async fn process_batch(&self) -> AppResult<usize> {
        let envelopes = self.store.fetch_unprocessed(self.batch_size).await?;

        if envelopes.is_empty() {
            return Ok(0);
        }

        let ids: Vec<uuid::Uuid> = envelopes.iter().map(|e| e.id).collect();

        self.broker.publish_batch(&envelopes).await?;
        self.store.mark_as_processed(&ids).await?;

        Ok(envelopes.len())
    }
}
