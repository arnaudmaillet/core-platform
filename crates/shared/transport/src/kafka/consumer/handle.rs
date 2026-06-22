use std::collections::HashMap;

use futures_util::StreamExt;
use rdkafka::{
    consumer::{Consumer, StreamConsumer},
    message::{Headers, Message},
    Offset, TopicPartitionList,
};
use tracing_opentelemetry::OpenTelemetrySpanExt;

use crate::{
    error::{CodecError, TransportError},
    kafka::{envelope::ConsumablePayload, error::KafkaTransportError},
    propagation::{carrier::extract_context, kafka::KafkaHeaderExtractor},
};

/// A single record consumed from Kafka, carrying both the typed payload (or the
/// decode error) **and** the offset coordinates required to commit it.
///
/// Unlike the producer-side [`crate::kafka::envelope::EventEnvelope`], a
/// `ConsumedMessage` always knows its `topic` / `partition` / `offset`, so the
/// worker can advance the consumer-group offset *past* it via
/// [`KafkaConsumerHandle::commit`] **after** processing has succeeded — the
/// foundation of at-least-once delivery.
///
/// # Why `payload` is a `Result`
///
/// A malformed (poison) record still yields a `ConsumedMessage` with
/// `payload = Err(..)`. This is deliberate: it lets the worker log the bad
/// record and commit past it, instead of being unable to skip it and blocking
/// the partition forever (head-of-line blocking). A broker/stream-level failure
/// — which has no offset to commit — is surfaced as the stream item's outer
/// `Err` instead.
pub struct ConsumedMessage<T> {
    /// Source topic.
    pub topic: String,
    /// Source partition.
    pub partition: i32,
    /// Record offset within the partition.
    pub offset: i64,
    /// Record key (empty string when absent or non-UTF-8).
    pub key: String,
    /// User headers, excluding the W3C trace-context headers.
    pub headers: HashMap<String, String>,
    /// Broker/producer timestamp in milliseconds, when available.
    pub timestamp_ms: Option<i64>,
    /// The original, undecoded record bytes (empty when the record had no payload).
    ///
    /// Retained so a poison record can be republished verbatim to a dead-letter
    /// topic — including a *decode* failure, where no typed `payload` exists.
    pub raw_payload: Vec<u8>,
    /// Decoded payload, or the decode error for a poison record.
    pub payload: Result<T, TransportError>,
}

/// A handle to a Kafka consumer that automatically extracts distributed trace context
/// from every incoming message and exposes a typed async stream of [`ConsumedMessage<T>`].
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

    /// Returns an infinite async stream of [`ConsumedMessage<T>`].
    ///
    /// Each poll extracts trace headers and sets the parent span before deserializing the
    /// payload, so all downstream `tracing` spans created while processing the message
    /// are automatically linked to the upstream producer's trace.
    ///
    /// # Offset management
    ///
    /// The stream never commits on the caller's behalf. After a message has been
    /// fully and successfully processed, call [`KafkaConsumerHandle::commit`] to
    /// advance the consumer-group offset past it. Until then the message remains
    /// uncommitted and will be redelivered if the consumer restarts — this is what
    /// makes delivery at-least-once. Configure the consumer with
    /// `enable_auto_commit = false` so nothing else advances the offset behind your
    /// back.
    ///
    /// A decode failure does **not** terminate the stream: the item is yielded with
    /// `payload = Err(..)` so the worker can commit past the poison record. Only a
    /// broker/stream-level error (which carries no offset) is surfaced as the item's
    /// outer `Err`.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let mut stream = handle.stream::<PostCreatedEvent>();
    /// while let Some(item) = stream.next().await {
    ///     let msg = item?; // broker error → propagate and restart the loop
    ///     match &msg.payload {
    ///         Ok(event) => {
    ///             process(event).await?;            // on failure: return WITHOUT committing
    ///             handle.commit(&msg)?;             // success → advance the offset
    ///         }
    ///         Err(err) => {
    ///             tracing::warn!(%err, "poison record — skipping");
    ///             handle.commit(&msg)?;             // skip past unparseable records
    ///         }
    ///     }
    /// }
    /// ```
    pub fn stream<T: ConsumablePayload>(
        &self,
    ) -> impl futures::Stream<Item = Result<ConsumedMessage<T>, TransportError>> + '_ {
        self.consumer.stream().map(|msg_result| {
            // A broker/stream-level error (rebalance, transport failure, …) has no
            // offset to commit. Surface it so the worker can restart its loop.
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

            // ── Payload deserialization (deferred error) ─────────────────────────────
            // A decode failure is captured in `payload` rather than aborting the stream,
            // so the worker can still see the offset and commit past the poison record.
            // The raw bytes are retained regardless, so the record can be forwarded to
            // a dead-letter topic even when it fails to decode.
            let raw_payload = msg.payload().map(<[u8]>::to_vec).unwrap_or_default();

            let payload = match msg.payload() {
                None => Err(TransportError::Kafka(KafkaTransportError::EmptyPayload)),
                Some(bytes) => serde_json::from_slice::<T>(bytes)
                    .map_err(|e| TransportError::Codec(CodecError::Json(e))),
            };

            let consumed = ConsumedMessage {
                topic: msg.topic().to_string(),
                partition: msg.partition(),
                offset: msg.offset(),
                key: msg
                    .key()
                    .and_then(|k| std::str::from_utf8(k).ok())
                    .unwrap_or("")
                    .to_string(),
                headers: user_headers,
                timestamp_ms: msg.timestamp().to_millis(),
                raw_payload,
                payload,
            };

            tracing::debug!(
                topic = %consumed.topic,
                partition = consumed.partition,
                offset = consumed.offset,
                key = %consumed.key,
                "Kafka message received"
            );

            Ok(consumed)
        })
    }

    /// Commits the offset *past* `msg`, marking it (and everything before it on the
    /// same partition) as processed.
    ///
    /// Call this only after the message has been fully and successfully handled — or
    /// to deliberately skip past a poison record. Committing `offset + 1` follows
    /// Kafka's "next offset to consume" convention, identical to `commit_message`.
    /// Uses async commit mode, so the call returns as soon as the request is queued;
    /// the offset is durably committed by the next group commit.
    pub fn commit<T>(&self, msg: &ConsumedMessage<T>) -> Result<(), TransportError> {
        let mut tpl = TopicPartitionList::new();
        tpl.add_partition_offset(&msg.topic, msg.partition, Offset::Offset(msg.offset + 1))
            .map_err(|e| TransportError::Kafka(KafkaTransportError::Config(e.to_string())))?;

        self.consumer
            .commit(&tpl, rdkafka::consumer::CommitMode::Async)
            .map_err(|e| TransportError::Kafka(KafkaTransportError::Consumer(e)))
    }
}
