// crates/shared-kernel/src/transport/kafka/consumer.rs

use crate::core::{Error, Result};
use crate::messaging::{EventConsumer, EventEnvelope, EventHandler};
use async_trait::async_trait;
use rdkafka::config::ClientConfig;
use rdkafka::consumer::{CommitMode, Consumer, StreamConsumer};
use rdkafka::message::Message;
use std::sync::Arc;
use tokio::sync::Semaphore;
use tokio_util::sync::CancellationToken;

pub struct KafkaEventConsumer {
    client_config: ClientConfig,
    shutdown_token: CancellationToken,
    concurrency_limit: Arc<Semaphore>,
}

impl KafkaEventConsumer {
    pub fn new(brokers: &str, group_id: &str, max_concurrency: usize) -> Self {
        let mut config = ClientConfig::new();
        config
            .set("bootstrap.servers", brokers)
            .set("group.id", group_id)
            // PASSAGE EN COMMIT MANUEL POUR ÉVITER LES PERTES DE DONNÉES
            .set("enable.auto.commit", "false")
            .set("auto.offset.reset", "earliest")
            .set("session.timeout.ms", "45000")
            .set("max.poll.interval.ms", "300000");

        Self {
            client_config: config,
            shutdown_token: CancellationToken::new(),
            concurrency_limit: Arc::new(Semaphore::new(max_concurrency)),
        }
    }

    pub fn stop(&self) {
        log::info!("Signaling Kafka consumer to stop...");
        self.shutdown_token.cancel();
    }
}

#[async_trait]
impl EventConsumer for KafkaEventConsumer {
    async fn consume(&self, topic: &str, handler: EventHandler) -> Result<()> {
        let consumer: StreamConsumer = self.client_config.create()?;
        let consumer = Arc::new(consumer);

        consumer
            .subscribe(&[topic])
            .map_err(|e| Error::internal(e.to_string()))?;

        let handler = Arc::new(handler);

        while !self.shutdown_token.is_cancelled() {
            tokio::select! {
                _ = self.shutdown_token.cancelled() => break,
                result = consumer.recv() => {
                    match result {
                        Ok(message) => {
                            let payload = match message.payload() {
                                Some(p) => p.to_vec(),
                                None => continue,
                            };

                            let topic_name = message.topic().to_string();
                            let partition = message.partition();
                            let next_offset = rdkafka::Offset::Offset(message.offset() + 1);

                            let h = Arc::clone(&handler);
                            let c = Arc::clone(&consumer);

                            let permit = self.concurrency_limit.clone().acquire_owned().await
                                .map_err(|e| Error::internal(e.to_string()))?;

                            tokio::spawn(async move {
                                match serde_json::from_slice::<EventEnvelope>(&payload) {
                                    Ok(envelope) => {
                                        if let Err(e) = (h)(envelope).await {
                                            log::error!("❌ Handler failed for event: {:?}", e);
                                        } else {
                                            let mut tpo = rdkafka::TopicPartitionList::new();
                                            if let Err(err) = tpo.add_partition_offset(&topic_name, partition, next_offset) {
                                                log::error!("⚠️ Failed to create partition list: {}", err);
                                            } else {
                                                if let Err(err) = c.commit(&tpo, CommitMode::Async) {
                                                    log::error!("⚠️ Failed to commit offset for topic {}, partition {}: {}", topic_name, partition, err);
                                                }
                                            }
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
