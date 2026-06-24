use rdkafka::{
    message::OwnedHeaders,
    producer::{FutureProducer, FutureRecord},
    util::Timeout,
};

use crate::{
    error::{CodecError, TransportError},
    kafka::{
        envelope::{EventEnvelope, PublishablePayload},
        error::KafkaTransportError,
    },
    propagation::{
        carrier::inject_context,
        kafka::KafkaHeaderInjector,
    },
};

/// A cheaply cloneable handle to a Kafka producer.
///
/// Holds an `Arc`-backed [`FutureProducer`] so handles can be shared across Tokio tasks
/// without additional synchronisation overhead.
///
/// # Trace context propagation
///
/// Every call to [`publish`] automatically injects the current `tracing` span's W3C
/// `traceparent` and `tracestate` into the Kafka record headers before produce.
/// The consumer counterpart extracts these headers and re-establishes the parent span,
/// giving end-to-end distributed traces across the async message boundary.
///
/// [`publish`]: KafkaProducerHandle::publish
#[derive(Clone)]
pub struct KafkaProducerHandle {
    producer: FutureProducer,
}

impl KafkaProducerHandle {
    pub(crate) fn new(producer: FutureProducer) -> Self {
        Self { producer }
    }

    /// Serialises `envelope.payload` to JSON and publishes the record to Kafka,
    /// injecting the current span's trace context into the record's headers.
    ///
    /// # Delivery guarantee
    ///
    /// The future resolves when the broker has acknowledged the produce request according
    /// to the `acks` setting in [`crate::kafka::config::producer::ProducerConfig`].
    /// With `acks = "all"` this means at-least-once delivery.
    ///
    /// # Errors
    ///
    /// Returns [`TransportError::Codec`] if serialisation fails.
    /// Returns [`TransportError::Kafka`] on broker-level produce failure.
    pub async fn publish<T: PublishablePayload>(
        &self,
        envelope: EventEnvelope<T>,
    ) -> Result<(), TransportError> {
        let (topic, key, payload, user_headers, timestamp_ms) = envelope.into_parts();

        let payload_bytes =
            serde_json::to_vec(&payload).map_err(|e| TransportError::Codec(CodecError::Json(e)))?;

        let headers = build_headers_with_trace(user_headers);

        let mut record = FutureRecord::to(&topic)
            .key(&key)
            .payload(&payload_bytes)
            .headers(headers);

        if let Some(ts) = timestamp_ms {
            record = record.timestamp(ts);
        }

        self.producer
            .send(record, Timeout::Never)
            .await
            .map_err(|(e, _msg)| TransportError::Kafka(KafkaTransportError::Producer(e)))?;

        tracing::debug!(topic = %topic, key = %key, "Kafka message published");

        Ok(())
    }

    /// Publishes a raw byte payload, bypassing JSON serialisation.
    ///
    /// Trace context is still injected automatically.
    pub async fn publish_raw(
        &self,
        topic: &str,
        key: &str,
        payload: &[u8],
        user_headers: std::collections::HashMap<String, String>,
    ) -> Result<(), TransportError> {
        let headers = build_headers_with_trace(user_headers);

        self.producer
            .send(
                FutureRecord::to(topic)
                    .key(key)
                    .payload(payload)
                    .headers(headers),
                Timeout::Never,
            )
            .await
            .map_err(|(e, _msg)| TransportError::Kafka(KafkaTransportError::Producer(e)))?;

        Ok(())
    }
}

/// Builds an [`OwnedHeaders`] instance from user-defined key-value pairs, then
/// injects the current span's W3C trace context as additional headers.
fn build_headers_with_trace(
    user_headers: std::collections::HashMap<String, String>,
) -> OwnedHeaders {
    use opentelemetry::propagation::Injector;

    let mut injector = user_headers
        .into_iter()
        .fold(KafkaHeaderInjector::new(), |mut inj, (k, v)| {
            // User-defined headers are added through the same Injector mechanism as trace
            // headers so that the OwnedHeaders builder pattern is managed in one place.
            inj.set(&k, v);
            inj
        });

    inject_context(&mut injector);

    injector.into_headers()
}
