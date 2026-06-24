use std::collections::HashMap;

use serde::{de::DeserializeOwned, Serialize};

/// A typed Kafka message wrapper used by both the producer and the consumer.
///
/// `T` is the domain payload type (must be serializable to send, deserializable to receive).
/// Trace context headers are NOT stored here — they are injected at publish time by
/// [`crate::kafka::producer::handle::KafkaProducerHandle`] and extracted at consume time
/// by [`crate::kafka::consumer::handle::KafkaConsumerHandle`].
///
/// # Custom headers
///
/// Any key-value pairs placed in `headers` before calling `publish` are included in the
/// Kafka record alongside the automatically-injected trace headers. Header values must be
/// valid UTF-8 strings.
#[derive(Debug, Clone)]
pub struct EventEnvelope<T> {
    /// Destination topic for producers; source topic for consumers.
    pub topic: String,

    /// Message key used by Kafka's partitioner. Using a stable domain identifier
    /// (e.g. post ID) ensures all events for the same entity land on the same partition,
    /// preserving ordering guarantees.
    pub key: String,

    /// Typed domain payload.
    pub payload: T,

    /// Arbitrary string headers included in the Kafka record. Trace context headers
    /// (`traceparent`, `tracestate`) are injected automatically and must not be set here.
    pub headers: HashMap<String, String>,

    /// Optional Unix timestamp in milliseconds. When `None`, the Kafka broker assigns the
    /// broker-side creation time.
    pub timestamp_ms: Option<i64>,
}

impl<T> EventEnvelope<T> {
    pub fn new(topic: impl Into<String>, key: impl Into<String>, payload: T) -> Self {
        Self {
            topic: topic.into(),
            key: key.into(),
            payload,
            headers: HashMap::new(),
            timestamp_ms: None,
        }
    }

    pub fn with_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert(key.into(), value.into());
        self
    }

    pub fn with_timestamp(mut self, ts_ms: i64) -> Self {
        self.timestamp_ms = Some(ts_ms);
        self
    }

    /// Converts this envelope to a serializable form, consuming `self`.
    /// Used internally by the producer handle.
    pub fn into_parts(self) -> (String, String, T, HashMap<String, String>, Option<i64>) {
        (
            self.topic,
            self.key,
            self.payload,
            self.headers,
            self.timestamp_ms,
        )
    }
}

/// Marker trait bound used by [`crate::kafka::producer::handle::KafkaProducerHandle`].
/// Every publishable payload must be serializable to JSON bytes.
pub trait PublishablePayload: Serialize + Send + Sync + 'static {}
impl<T: Serialize + Send + Sync + 'static> PublishablePayload for T {}

/// Marker trait bound used by [`crate::kafka::consumer::handle::KafkaConsumerHandle`].
/// Every receivable payload must be deserializable from JSON bytes.
pub trait ConsumablePayload: DeserializeOwned + Send + Sync + 'static {}
impl<T: DeserializeOwned + Send + Sync + 'static> ConsumablePayload for T {}
