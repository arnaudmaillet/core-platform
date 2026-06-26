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
use crate::application::port::SubjectCipher;
use crate::domain::SubjectPseudonym;
use crate::error::AuditError;
use crate::infrastructure::auth_decode::{AuthEventWire, map_session_issued, map_session_revoked};
use crate::infrastructure::decode::{AuditEventWire, map_audit_event};
use crate::infrastructure::moderation_decode::{
    ModerationEventWire, map_decision_recorded, map_enforcement_applied,
};

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

/// Run the `auth.v1.events` ingest consumer until the stream ends. Auth events
/// carry no free-text PII, so there is no sealing — `session_issued` /
/// `session_revoked` map directly and chain; every other auth event is a benign
/// committed skip.
pub async fn run_auth_ingest_consumer(
    consumer: KafkaConsumerHandle,
    producer: KafkaProducerHandle,
    handler: Arc<IngestHandler>,
) {
    tracing::info!("audit auth ingest consumer started");
    let policy = RetryPolicy::default();
    let result = run_consumer::<AuthEventWire, _>(&consumer, &producer, &policy, move |wire| {
        let handler = Arc::clone(&handler);
        let wire = wire.clone();
        Box::pin(async move {
            let outcome = async {
                match wire {
                    AuthEventWire::SessionIssued(issued) => {
                        handler.ingest(map_session_issued(&issued)?).await?;
                    }
                    AuthEventWire::SessionRevoked(revoked) => {
                        handler.ingest(map_session_revoked(&revoked)?).await?;
                    }
                    AuthEventWire::Other => {}
                }
                Ok::<(), AuditError>(())
            }
            .await;
            ProcessOutcome::from_result(outcome)
        })
    })
    .await;
    if let Err(e) = result {
        tracing::error!(error = %e, "audit auth ingest consumer stopped");
    }
}

/// Run the `moderation.v1.events` ingest consumer until the stream ends.
///
/// For a `decision_recorded` it seals the rationale into a crypto-shreddable
/// envelope (the `cipher`) before mapping + chaining; for an `enforcement_applied`
/// it maps directly (no PII); every other moderation event is a benign skip that
/// still commits the offset. The cipher's key-vault faults are retryable (`run_consumer`
/// retries without committing); a decode/domain fault is poison and dead-letters.
pub async fn run_moderation_ingest_consumer(
    consumer: KafkaConsumerHandle,
    producer: KafkaProducerHandle,
    handler: Arc<IngestHandler>,
    cipher: Arc<dyn SubjectCipher>,
) {
    tracing::info!("audit moderation ingest consumer started");
    let policy = RetryPolicy::default();
    let result = run_consumer::<ModerationEventWire, _>(&consumer, &producer, &policy, move |wire| {
        let handler = Arc::clone(&handler);
        let cipher = Arc::clone(&cipher);
        let wire = wire.clone();
        Box::pin(async move {
            let outcome = async {
                match wire {
                    ModerationEventWire::DecisionRecorded(decision) => {
                        let subject = SubjectPseudonym::new(decision.subject.actor_id.clone())?;
                        // Seal the DSA rationale into a crypto-shreddable envelope.
                        let sealed = cipher.seal(&subject, &decision.rationale).await?;
                        let event = map_decision_recorded(&decision, sealed)?;
                        handler.ingest(event).await?;
                    }
                    ModerationEventWire::EnforcementApplied(enforcement) => {
                        let event = map_enforcement_applied(&enforcement)?;
                        handler.ingest(event).await?;
                    }
                    // Out-of-scope moderation event: a harmless committed skip.
                    ModerationEventWire::Other => {}
                }
                Ok::<(), AuditError>(())
            }
            .await;
            ProcessOutcome::from_result(outcome)
        })
    })
    .await;
    if let Err(e) = result {
        tracing::error!(error = %e, "audit moderation ingest consumer stopped");
    }
}
