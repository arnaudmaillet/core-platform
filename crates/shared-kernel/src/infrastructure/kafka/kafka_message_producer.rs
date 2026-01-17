// crates/shared-kernel/src/infrastructure/kafka/kafka_message_producer.rs

use async_trait::async_trait;
use rdkafka::config::ClientConfig;
use rdkafka::producer::{FutureProducer, FutureRecord};
use std::time::Duration;
use rdkafka::message::{Header, OwnedHeaders};
use crate::application::ports::MessageProducer;
use crate::domain::events::EventEnvelope;
use crate::errors::{AppResult, AppError, ErrorCode};

pub struct KafkaMessageProducer {
    producer: FutureProducer,
    default_topic: String,
}

impl KafkaMessageProducer {
    pub async fn new(brokers: &str, default_topic: String) -> AppResult<Self> {
        let producer: FutureProducer = ClientConfig::new()
            .set("bootstrap.servers", brokers)
            .set("message.timeout.ms", "5000")
            // --- OPTIMISATIONS  ---
            .set("compression.type", "snappy") // Compromis idéal CPU/Taille
            .set("acks", "all")                // Sécurité maximale
            .set("queue.buffering.max.ms", "5") // Attente minime pour grouper les messages
            .set("batch.num.messages", "1000")  // Taille de batch idéale
            .set("linger.ms", "10")             // Laisse le temps au batch de se remplir
            .create()
            .map_err(|e| AppError::new(ErrorCode::InternalError, format!("Kafka config error: {e}")))?;

        Ok(Self { producer, default_topic })
    }
}

    #[async_trait]
impl MessageProducer for KafkaMessageProducer {
    async fn publish(&self, event: &EventEnvelope) -> AppResult<()> {
        let payload = serde_json::to_string(event)
            .map_err(|e| AppError::new(ErrorCode::InternalError, e.to_string()))?;

        let record = FutureRecord::to(&self.default_topic)
            .payload(&payload)
            .key(&event.aggregate_id)
            .headers(OwnedHeaders::new()
                .insert(Header {
                    key: "event_type",
                    value: Some(&event.event_type), // ex: "account.created"
                })
            );

        self.producer
            .send(record, Duration::from_secs(5))
            .await
            .map_err(|(e, _)| AppError::from(e))?;

        Ok(())
    }

        async fn publish_batch(&self, events: &[EventEnvelope]) -> AppResult<()> {
            // 1. On pré-sérialise tout.
            // On doit posséder ces Strings pour qu'elles ne soient pas détruites
            // pendant que les futures tournent.
            let payloads: Vec<String> = events
                .iter()
                .map(|e| serde_json::to_string(e).unwrap_or_default())
                .collect();

            let mut futures = Vec::with_capacity(events.len());

            // 2. On envoie vers le buffer interne de librdkafka
            for (i, event) in events.iter().enumerate() {
                let record = FutureRecord::to(&self.default_topic)
                    .payload(&payloads[i])
                    .key(&event.aggregate_id)
                    .headers(OwnedHeaders::new()
                        .insert(Header {
                            key: "event_type",
                            value: Some(&event.event_type),
                        })
                    );
                futures.push(self.producer.send(record, Duration::from_secs(0)));
            }

            // 3. On attend les confirmations.
            // 'payloads' est toujours vivant ici, donc les références sont valides.
            for future in futures {
                future.await.map_err(|(e, _)| AppError::from(e))?;
            }

            Ok(())
        }
}
