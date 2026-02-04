// crates/shared-kernel/src/infrastructure/messaging/kafka_consumer.rs

use crate::application::ports::{MessageConsumer, MessageHandler};
use crate::domain::events::EventEnvelope;
use crate::errors::{AppError, AppResult, ErrorCode};
use async_trait::async_trait;
use rdkafka::config::ClientConfig;
use rdkafka::consumer::{Consumer, StreamConsumer};
use rdkafka::message::Message;
use std::sync::Arc;
use tokio::sync::Semaphore;
use tokio_util::sync::CancellationToken;

pub struct KafkaMessageConsumer {
    client_config: ClientConfig,
    shutdown_token: CancellationToken,
    // Limite le nombre de messages traités en parallèle (ex: 1000)
    concurrency_limit: Arc<Semaphore>,
}

impl KafkaMessageConsumer {
    pub fn new(brokers: &str, group_id: &str, max_concurrency: usize) -> Self {
        let mut config = ClientConfig::new();
        config
            .set("bootstrap.servers", brokers)
            .set("group.id", group_id)
            .set("enable.auto.commit", "true")
            .set("auto.commit.interval.ms", "5000") // Commit toutes les 5s
            .set("auto.offset.reset", "earliest") // Ne rate rien au démarrage
            // Sécurité pour ne pas perdre de messages si le processing est lent
            .set("session.timeout.ms", "45000")
            .set("max.poll.interval.ms", "300000");

        Self {
            client_config: config,
            shutdown_token: CancellationToken::new(),
            concurrency_limit: Arc::new(Semaphore::new(max_concurrency)),
        }
    }

    pub fn stop(&self) {
        log::info!("Sinaling Kafka consumer to stop...");
        self.shutdown_token.cancel();
    }
}

#[async_trait]
impl MessageConsumer for KafkaMessageConsumer {
    async fn consume(&self, topic: &str, handler: MessageHandler) -> AppResult<()> {
        let consumer: StreamConsumer = self.client_config.create()?;
        consumer
            .subscribe(&[topic])
            .map_err(|e| AppError::new(ErrorCode::InternalError, e.to_string()))?;

        let handler = Arc::new(handler);

        while !self.shutdown_token.is_cancelled() {
            tokio::select! {
                _ = self.shutdown_token.cancelled() => break,
                result = consumer.recv() => {
                    match result {
                        Ok(message) => {
                            // Extraction propre du payload
                            let payload = match message.payload() {
                                Some(p) => p.to_vec(),
                                None => continue,
                            };

                            // On récupère le header 'event_type' si besoin (optionnel ici)
                            // let _event_type = message.headers().and_then(|h| h.get("event_type"));

                            let h = Arc::clone(&handler);
                            let permit = self.concurrency_limit.clone().acquire_owned().await
                                .map_err(|e| AppError::new(ErrorCode::InternalError, e.to_string()))?;

                            tokio::spawn(async move {
                                // Désérialisation
                                match serde_json::from_slice::<EventEnvelope>(&payload) {
                                    Ok(envelope) => {
                                        if let Err(e) = (h)(envelope).await {
                                            log::error!("❌ Handler failed for event: {:?}", e);
                                        }
                                    },
                                    Err(e) => log::error!("⚠️ Failed to deserialize envelope: {}", e),
                                }
                                drop(permit);
                            });
                        },
                        Err(e) => log::error!("Kafka receive error: {}", e),
                    }
                }
            }
        }
        log::info!("🛑 Kafka consumer loop stopped.");
        Ok(())
    }
}
