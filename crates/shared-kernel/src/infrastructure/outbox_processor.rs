// crates/shared-kernel/src/application/outbox_processor.rs

use std::time::Duration;
use tokio::time::sleep;
use crate::application::ports::MessageProducer;
// On ne dépend plus de DomainEvent ici, car on manipule du JSON
use crate::domain::repositories::OutboxStore;
use crate::errors::AppResult;

pub struct OutboxProcessor<Store, Broker>
where
    Store: OutboxStore,
    Broker: MessageProducer
{
    store: Store,
    broker: Broker,
    batch_size: u32,
    polling_interval: Duration,
}

impl<Store, Broker> OutboxProcessor<Store, Broker>
where
    Store: OutboxStore,
    Broker: MessageProducer
{
    pub fn new(store: Store, broker: Broker, batch_size: u32, interval: Duration) -> Self {
        Self { store, broker, batch_size, polling_interval: interval }
    }

    pub async fn run(&self) -> ! {
        loop {
            match self.process_batch().await {
                Ok(0) => {
                    // File vide, on dort
                    sleep(self.polling_interval).await;
                }
                Ok(count) => {
                    tracing::info!("Relayed {} events", count);
                    // Si on a traité un batch plein, on ne dort pas,
                    // on reboucle immédiatement pour vider le backlog
                    if count < self.batch_size as usize {
                        sleep(self.polling_interval).await;
                    }
                }
                Err(e) => {
                    tracing::error!("Relay error: {:?}", e);
                    sleep(self.polling_interval).await;
                }
            }
        }
    }

    async fn process_batch(&self) -> AppResult<usize> {
        // fetch_unprocessed() renvoie Result<..., DomainError>
        // Le "?" le transforme automatiquement en AppError. MAGIE !
        let envelopes = self.store.fetch_unprocessed(self.batch_size).await?;

        if envelopes.is_empty() { return Ok(0); }

        let ids: Vec<uuid::Uuid> = envelopes.iter().map(|e| e.id).collect();

        // publish_batch() renvoie déjà AppResult (AppError)
        self.broker.publish_batch(&envelopes).await?;

        // mark_as_processed() renvoie Result<..., DomainError> -> promu en AppError
        self.store.mark_as_processed(&ids).await?;

        Ok(envelopes.len())
    }
}