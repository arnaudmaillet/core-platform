//! The outbound popularity signal over Kafka (`counter.v1.popularity`).
//!
//! The only thing counter-analytics publishes. A coarse, slow-loop emission
//! consumed by `search` (its `PopularityScore` ranking input) and `timeline`.

use async_trait::async_trait;
use serde::Serialize;
use transport::error::TransportError;
use transport::kafka::envelope::EventEnvelope;
use transport::kafka::producer::handle::KafkaProducerHandle;

use crate::application::port::SignalPublisher;
use crate::domain::{EntityRef, PopularityScore};
use crate::error::CounterError;

const TOPIC_POPULARITY: &str = "counter.v1.popularity";

/// The wire payload of a popularity snapshot. Deliberately tiny: a reference and a
/// coarse score — no per-actor data, nothing volatile.
#[derive(Debug, Clone, Serialize)]
pub struct PopularityEvent {
    pub entity_type: String,
    pub entity_id: String,
    pub score: f64,
}

fn transport_err(e: TransportError) -> CounterError {
    CounterError::SignalPublishFailed {
        reason: e.to_string(),
    }
}

pub struct KafkaSignalPublisher {
    producer: KafkaProducerHandle,
}

impl KafkaSignalPublisher {
    pub fn new(producer: KafkaProducerHandle) -> Self {
        Self { producer }
    }
}

#[async_trait]
impl SignalPublisher for KafkaSignalPublisher {
    async fn publish_popularity(
        &self,
        entity: &EntityRef,
        score: PopularityScore,
    ) -> Result<(), CounterError> {
        let key = format!("{}:{}", entity.kind.as_str(), entity.id.as_str());
        let payload = PopularityEvent {
            entity_type: entity.kind.as_str().to_owned(),
            entity_id: entity.id.as_str().to_owned(),
            score: score.value(),
        };
        let envelope = EventEnvelope::new(TOPIC_POPULARITY, key, payload)
            .with_header("entity_type", entity.kind.as_str());
        self.producer.publish(envelope).await.map_err(transport_err)
    }
}
