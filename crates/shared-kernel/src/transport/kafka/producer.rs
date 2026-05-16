// crates/shared-kernel/src/transport/kafka/producer.rs

use crate::core::{Error, ErrorCode, Result};
use crate::messaging::{EventEnvelope, EventProducer};
use async_trait::async_trait;
use rdkafka::config::ClientConfig;
use rdkafka::message::{Header, OwnedHeaders};
use rdkafka::producer::{FutureProducer, FutureRecord};
use std::time::Duration;

pub struct KafkaEventProducer {
    producer: FutureProducer,
    default_topic: String,
}

impl KafkaEventProducer {
    pub async fn new(brokers: &str, default_topic: String) -> Result<Self> {
        let producer: FutureProducer = ClientConfig::new()
            .set("bootstrap.servers", brokers)
            .set("message.timeout.ms", "5000")
            // --- OPTIMISATIONS DE PRODUCTION ---
            .set("compression.type", "snappy")
            .set("acks", "all")
            // GARANTIE DU SHARDING : Aligne le calcul de hash par clé de librdkafka avec Java/Go
            .set("partitioner", "consistent_random")
            .set("queue.buffering.max.ms", "5")
            .set("batch.num.messages", "1000")
            .set("linger.ms", "10")
            .create()
            .map_err(|e| Error::internal(format!("Kafka config error: {}", e.to_string())))?;

        Ok(Self {
            producer,
            default_topic,
        })
    }
}

#[async_trait]
impl EventProducer for KafkaEventProducer {
    async fn publish(&self, event: &EventEnvelope) -> Result<()> {
        let payload = serde_json::to_string(event)
            .map_err(|e| Error::new(ErrorCode::InternalError, e.to_string()))?;

        let record = FutureRecord::to(&self.default_topic)
            .payload(&payload)
            .key(&event.aggregate_id)
            .headers(OwnedHeaders::new().insert(Header {
                key: "event_type",
                value: Some(&event.event_type),
            }));

        self.producer
            .send(record, Duration::from_secs(5))
            .await
            .map_err(|(e, _)| Error::from(e))?;

        Ok(())
    }

    async fn publish_batch(&self, events: &[EventEnvelope]) -> Result<()> {
        let payloads: Vec<String> = events
            .iter()
            .map(|e| serde_json::to_string(e).unwrap_or_default())
            .collect();

        let mut futures = Vec::with_capacity(events.len());

        for (i, event) in events.iter().enumerate() {
            let record = FutureRecord::to(&self.default_topic)
                .payload(&payloads[i])
                .key(&event.aggregate_id)
                .headers(OwnedHeaders::new().insert(Header {
                    key: "event_type",
                    value: Some(&event.event_type),
                }));

            futures.push(self.producer.send(record, Duration::from_secs(5))); // Changé 0 à 5s pour éviter les blocages silencieux
        }

        for future in futures {
            future.await.map_err(|(e, _)| Error::from(e))?;
        }

        Ok(())
    }
}
