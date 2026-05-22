// crates/infra-kafka/src/consumer.rs

use async_trait::async_trait;
use rdkafka::config::ClientConfig;
use rdkafka::consumer::{CommitMode, Consumer, StreamConsumer};
use rdkafka::message::Message;
use shared_kernel::core::{Error, Result};
use shared_kernel::messaging::{EventConsumer, EventEnvelope, EventHandler};
use std::sync::Arc;
use tokio::sync::Semaphore;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, instrument, warn};

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
        info!("Signaling Kafka consumer to stop...");
        self.shutdown_token.cancel();
    }
}

#[async_trait]
impl EventConsumer for KafkaEventConsumer {
    // instrument capture automatiquement le paramètre `topic` dans tous les logs de cette fonction
    #[instrument(skip(self, handler), fields(kafka.topic = %topic))]
    async fn consume(&self, topic: &str, handler: EventHandler) -> Result<()> {
        let consumer: StreamConsumer = self.client_config.create()?;
        let consumer = Arc::new(consumer);

        consumer
            .subscribe(&[topic])
            .map_err(|e| Error::internal(e.to_string()))?;

        let handler = Arc::new(handler);
        info!("Kafka consumer loop successfully started and subscribed");

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

                            // On attache le Span actuel au contexte asynchrone du spawn
                            let spawn_span = tracing::Span::current();

                            tokio::spawn(async move {
                                let _enter = spawn_span.enter();

                                match serde_json::from_slice::<EventEnvelope>(&payload) {
                                    Ok(envelope) => {
                                        // Capture de l'aggregate_id pour l'associer au log du handler
                                        let aggregate_id = envelope.aggregate_id.clone();

                                        if let Err(e) = (h)(envelope).await {
                                            error!(error = ?e, aggregate_id = %aggregate_id, "Handler failed for event");
                                        } else {
                                            let mut tpo = rdkafka::TopicPartitionList::new();
                                            if let Err(err) = tpo.add_partition_offset(&topic_name, partition, next_offset) {
                                                error!(error = %err, partition, "Failed to create partition list for commit");
                                            } else if let Err(err) = c.commit(&tpo, CommitMode::Async) {
                                                warn!(error = %err, partition, "Failed to commit offset");
                                            }
                                        }
                                    },
                                    Err(e) => error!(error = %e, "Failed to deserialize event envelope"),
                                }
                                drop(permit);
                            });
                        },
                        Err(e) => error!(error = %e, "Kafka receive error encountered"),
                    }
                }
            }
        }

        info!("Kafka consumer loop stopped cleanly.");
        Ok(())
    }
}
