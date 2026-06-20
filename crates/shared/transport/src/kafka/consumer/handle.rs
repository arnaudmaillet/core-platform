use std::collections::HashMap;

use futures_util::StreamExt;
use rdkafka::{
    consumer::StreamConsumer,
    message::{Headers, Message},
};
use tracing_opentelemetry::OpenTelemetrySpanExt;

use crate::{
    error::{CodecError, TransportError},
    kafka::{
        envelope::{ConsumablePayload, EventEnvelope},
        error::KafkaTransportError,
    },
    propagation::{carrier::extract_context, kafka::KafkaHeaderExtractor},
};

/// A handle to a Kafka consumer that automatically extracts distributed trace context
/// from every incoming message and exposes a typed async stream of [`EventEnvelope<T>`].
///
/// # Trace context propagation
///
/// For each received message, the handle:
/// 1. Extracts `traceparent` / `tracestate` from the Kafka record headers.
/// 2. Reconstructs the remote [`opentelemetry::Context`].
/// 3. Sets it as the parent of the current `tracing` span, establishing a continuous
///    distributed trace from the producer to this consumer.
pub struct KafkaConsumerHandle {
    consumer: StreamConsumer,
}

impl KafkaConsumerHandle {
    pub(crate) fn new(consumer: StreamConsumer) -> Self {
        Self { consumer }
    }

    /// Returns an infinite async stream of deserialized [`EventEnvelope<T>`].
    ///
    /// Each poll extracts trace headers and sets the parent span before deserializing the
    /// payload, so all downstream `tracing` spans created while processing the message
    /// are automatically linked to the upstream producer's trace.
    ///
    /// # Offset management
    ///
    /// With `enable_auto_commit = false` (recommended), callers must commit offsets
    /// explicitly after successful processing. Use [`KafkaConsumerHandle::commit`] or
    /// [`rdkafka::consumer::Consumer::commit_message`] directly.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let mut stream = handle.stream::<PostCreatedEvent>();
    /// while let Some(result) = stream.next().await {
    ///     let envelope = result?;
    ///     process(envelope.payload).await?;
    /// }
    /// ```
    pub fn stream<T: ConsumablePayload>(
        &self,
    ) -> impl futures::Stream<Item = Result<EventEnvelope<T>, TransportError>> + '_ {
        self.consumer.stream().map(|msg_result| {
            let msg = msg_result
                .map_err(|e| TransportError::Kafka(KafkaTransportError::Consumer(e)))?;

            // ── Trace context extraction ─────────────────────────────────────────────
            // Extract the remote context and set it as the parent of the current span.
            // Any `tracing` span opened after this point within the same task will be a
            // child of the upstream producer's span.
            if let Some(headers) = msg.headers() {
                let parent_cx = extract_context(&KafkaHeaderExtractor(headers));
                tracing::Span::current().set_parent(parent_cx);
            }

            // ── Payload deserialization ──────────────────────────────────────────────
            let payload_bytes = msg
                .payload()
                .ok_or(TransportError::Kafka(KafkaTransportError::EmptyPayload))?;

            let payload: T = serde_json::from_slice(payload_bytes)
                .map_err(|e| TransportError::Codec(CodecError::Json(e)))?;

            // ── Reconstruct user headers (excluding trace headers) ───────────────────
            let user_headers: HashMap<String, String> = msg
                .headers()
                .map(|h| {
                    (0..h.count())
                        .filter_map(|i| {
                            let header = h.get(i);
                            // Skip W3C trace context headers — those are transport-internal.
                            if header.key == "traceparent" || header.key == "tracestate" {
                                return None;
                            }
                            let value = header
                                .value
                                .and_then(|v| std::str::from_utf8(v).ok())
                                .unwrap_or("")
                                .to_string();
                            Some((header.key.to_string(), value))
                        })
                        .collect()
                })
                .unwrap_or_default();

            let envelope = EventEnvelope {
                topic: msg.topic().to_string(),
                key: msg
                    .key()
                    .and_then(|k| std::str::from_utf8(k).ok())
                    .unwrap_or("")
                    .to_string(),
                payload,
                headers: user_headers,
                timestamp_ms: msg.timestamp().to_millis(),
            };

            tracing::debug!(
                topic = %envelope.topic,
                key = %envelope.key,
                "Kafka message received and deserialized"
            );

            Ok(envelope)
        })
    }

    /// Commits the offset for `msg` asynchronously.
    ///
    /// Pass the original `BorrowedMessage` from the rdkafka stream. Use this when
    /// `enable_auto_commit = false` (the default) to advance the consumer group offset
    /// after successful processing.
    pub fn commit<'a>(
        &'a self,
        msg: &rdkafka::message::BorrowedMessage<'a>,
    ) -> Result<(), TransportError> {
        use rdkafka::consumer::Consumer;
        self.consumer
            .commit_message(msg, rdkafka::consumer::CommitMode::Async)
            .map_err(|e| TransportError::Kafka(KafkaTransportError::Consumer(e)))
    }
}
