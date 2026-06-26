//! The async ingest lane's Kafka wiring — the worker's command side.
//!
//! Runs on the shared at-least-once [`run_consumer`] runner (manual commit after a
//! terminal outcome, bounded retry + jitter, DLQ on poison/exhaustion). A decoded
//! [`AuditEventWire`] is distilled into a domain event and chained via the
//! [`IngestHandler`]; `run_consumer` owns deserialization, so poison bytes
//! dead-letter before reaching the mapping.
//!
//! Zero-loss: the offset is committed only after the event is durably persisted and
//! chained. A store fault is retryable (the runner retries without committing); a
//! decode/category/outcome fault is poison and dead-letters. A duplicate replay is
//! a benign success (`IngestHandler::ingest` dedupes), so the offset advances.

use std::sync::Arc;

use transport::kafka::consumer::{ProcessOutcome, RetryPolicy, run_consumer, KafkaConsumerHandle};
use transport::kafka::producer::KafkaProducerHandle;

use crate::application::IngestHandler;
use crate::error::AuditError;
use crate::infrastructure::decode::{AuditEventWire, map_audit_event};

/// Lets the at-least-once runner classify a failure: delegate to the error's own
/// retry verdict — transient store faults retry; decode / domain faults are poison
/// and dead-letter immediately.
impl transport::kafka::consumer::ClassifyError for AuditError {
    fn is_retryable(&self) -> bool {
        <Self as error::AppError>::is_retryable(self)
    }
}

/// Run the `audit.v1.events` ingest consumer until the stream ends.
pub async fn run_audit_ingest_consumer(
    consumer: KafkaConsumerHandle,
    producer: KafkaProducerHandle,
    handler: Arc<IngestHandler>,
) {
    tracing::info!("audit ingest consumer started");
    let policy = RetryPolicy::default();
    let result = run_consumer::<AuditEventWire, _>(&consumer, &producer, &policy, move |wire| {
        let handler = Arc::clone(&handler);
        let wire = wire.clone();
        Box::pin(async move {
            let outcome = async {
                let event = map_audit_event(wire)?;
                handler.ingest(event).await?;
                Ok::<(), AuditError>(())
            }
            .await;
            ProcessOutcome::from_result(outcome)
        })
    })
    .await;
    if let Err(e) = result {
        tracing::error!(error = %e, "audit ingest consumer stopped");
    }
}
