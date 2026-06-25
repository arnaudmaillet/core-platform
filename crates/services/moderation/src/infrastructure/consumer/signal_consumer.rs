use std::sync::Arc;

use chrono::Utc;
use cqrs::Envelope;
use serde::Deserialize;
use tracing::{error, info};
use uuid::Uuid;

use transport::kafka::consumer::{run_consumer, KafkaConsumerHandle, ProcessOutcome, RetryPolicy};
use transport::kafka::producer::KafkaProducerHandle;

use crate::application::command::{IngestSignalCommand, IngestSignalHandler};
use crate::domain::value_object::{ActorId, Confidence, EntityType, PolicyCategory, SubjectRef};
use crate::error::ModerationError;

/// Payload on `moderation.signals` (a classifier verdict).
#[derive(Debug, Deserialize)]
struct SignalEvent {
    entity_type: String,
    entity_id: String,
    actor_id: String,
    #[serde(default)]
    surface: String,
    source: String,
    category: String,
    confidence: f64,
}

/// Runs the classifier-signal consumer on the shared at-least-once runner.
pub async fn run_signal_consumer(
    consumer: KafkaConsumerHandle,
    handler: Arc<IngestSignalHandler>,
    producer: KafkaProducerHandle,
) {
    info!("moderation signal consumer started");
    let policy = RetryPolicy::default();
    let result = run_consumer::<SignalEvent, _>(&consumer, &producer, &policy, move |event| {
        let handler = Arc::clone(&handler);
        Box::pin(async move { process(handler.as_ref(), event).await })
    })
    .await;
    if let Err(e) = result {
        error!(error = %e, "moderation signal consumer stopped");
    }
}

async fn process(handler: &IngestSignalHandler, event: &SignalEvent) -> ProcessOutcome {
    let cmd = match build_command(event) {
        Ok(cmd) => cmd,
        Err(e) => return ProcessOutcome::from_result(Err::<(), _>(e)),
    };
    let result = handler.handle(Envelope::new(Uuid::now_v7(), cmd), Utc::now()).await.map(|_| ());
    ProcessOutcome::from_result(result)
}

fn build_command(event: &SignalEvent) -> Result<IngestSignalCommand, ModerationError> {
    let subject = SubjectRef::new(
        EntityType::try_from(event.entity_type.as_str())?,
        event.entity_id.clone(),
        ActorId::try_from(event.actor_id.as_str())?,
        event.surface.clone(),
    )?;
    Ok(IngestSignalCommand {
        subject,
        source: event.source.clone(),
        category: PolicyCategory::try_from(event.category.as_str())?,
        confidence: Confidence::clamped(event.confidence),
    })
}
