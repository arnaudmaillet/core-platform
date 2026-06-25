use std::sync::Arc;

use chrono::Utc;
use cqrs::Envelope;
use serde::Deserialize;
use tracing::{error, info};
use uuid::Uuid;

use transport::kafka::consumer::{run_consumer, KafkaConsumerHandle, ProcessOutcome, RetryPolicy};
use transport::kafka::producer::KafkaProducerHandle;

use crate::application::command::{IngestReportCommand, IngestReportHandler};
use crate::domain::value_object::{ActorId, EntityType, PolicyCategory, SubjectRef};
use crate::error::ModerationError;

/// Payload on `moderation.reports` (a user-submitted abuse report).
#[derive(Debug, Deserialize)]
struct ReportEvent {
    reporter_id: String,
    entity_type: String,
    entity_id: String,
    actor_id: String,
    #[serde(default)]
    surface: String,
    category: String,
    #[serde(default)]
    reason: String,
}

/// Runs the report consumer on the shared at-least-once runner.
pub async fn run_report_consumer(
    consumer: KafkaConsumerHandle,
    handler: Arc<IngestReportHandler>,
    producer: KafkaProducerHandle,
) {
    info!("moderation report consumer started");
    let policy = RetryPolicy::default();
    let result = run_consumer::<ReportEvent, _>(&consumer, &producer, &policy, move |event| {
        let handler = Arc::clone(&handler);
        Box::pin(async move { process(handler.as_ref(), event).await })
    })
    .await;
    if let Err(e) = result {
        error!(error = %e, "moderation report consumer stopped");
    }
}

async fn process(handler: &IngestReportHandler, event: &ReportEvent) -> ProcessOutcome {
    let cmd = match build_command(event) {
        Ok(cmd) => cmd,
        Err(e) => return ProcessOutcome::from_result(Err::<(), _>(e)),
    };
    let result = handler.handle(Envelope::new(Uuid::now_v7(), cmd), Utc::now()).await.map(|_| ());
    ProcessOutcome::from_result(result)
}

fn build_command(event: &ReportEvent) -> Result<IngestReportCommand, ModerationError> {
    let subject = SubjectRef::new(
        EntityType::try_from(event.entity_type.as_str())?,
        event.entity_id.clone(),
        ActorId::try_from(event.actor_id.as_str())?,
        event.surface.clone(),
    )?;
    Ok(IngestReportCommand {
        reporter_id: ActorId::try_from(event.reporter_id.as_str())?,
        subject,
        category: PolicyCategory::try_from(event.category.as_str())?,
        reason: event.reason.clone(),
    })
}
