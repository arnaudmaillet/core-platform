use std::sync::Arc;

use chrono::Utc;
use cqrs::Envelope;
use serde::Deserialize;
use tracing::{error, info};
use uuid::Uuid;

use transport::kafka::consumer::{run_consumer, KafkaConsumerHandle, ProcessOutcome, RetryPolicy};
use transport::kafka::producer::KafkaProducerHandle;

use crate::application::command::{ApplyModerationCommand, ApplyModerationHandler, ModerationAction};
use crate::domain::value_object::AssetId;
use crate::error::MediaError;

/// Lenient wire view of `moderation.v1.events` — media owns its own read schema and
/// must not depend on the `moderation` crate (a sideways edge). The serde tag is the
/// snake_case variant name (e.g. `enforcement_applied`).
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ModerationWireEvent {
    EnforcementApplied { subject: WireSubject, action: String },
    EnforcementReversed { subject: WireSubject },
    #[serde(other)]
    Other,
}

#[derive(Debug, Deserialize)]
struct WireSubject {
    entity_type: String,
    entity_id: String,
}

/// Runs the moderation consumer: maps an enforcement on a *media* subject into a
/// quarantine (content removal / visibility limit) or a restore (reversal).
pub async fn run_moderation_consumer(
    consumer: KafkaConsumerHandle,
    handler: Arc<ApplyModerationHandler>,
    producer: KafkaProducerHandle,
) {
    info!("media moderation consumer started");
    let policy = RetryPolicy::default();
    let result =
        run_consumer::<ModerationWireEvent, _>(&consumer, &producer, &policy, move |event| {
            let handler = Arc::clone(&handler);
            Box::pin(async move { process(handler.as_ref(), event).await })
        })
        .await;
    if let Err(e) = result {
        error!(error = %e, "media moderation consumer stopped");
    }
}

async fn process(handler: &ApplyModerationHandler, event: &ModerationWireEvent) -> ProcessOutcome {
    let Some((asset_id, action)) = map(event) else {
        // Not a media enforcement (or unparseable id) — committed no-op.
        return ProcessOutcome::from_result(Ok::<(), MediaError>(()));
    };
    let result = handler
        .handle(Envelope::new(Uuid::now_v7(), ApplyModerationCommand { asset_id, action }), Utc::now())
        .await
        .map(|_| ());
    ProcessOutcome::from_result(result)
}

/// Distills a moderation event into a media action, or `None` to skip.
fn map(event: &ModerationWireEvent) -> Option<(AssetId, ModerationAction)> {
    match event {
        ModerationWireEvent::EnforcementApplied { subject, action }
            if subject.entity_type == "media"
                && matches!(action.as_str(), "remove_content" | "visibility_limit") =>
        {
            AssetId::try_from(subject.entity_id.as_str())
                .ok()
                .map(|id| (id, ModerationAction::Quarantine))
        }
        ModerationWireEvent::EnforcementReversed { subject } if subject.entity_type == "media" => {
            AssetId::try_from(subject.entity_id.as_str())
                .ok()
                .map(|id| (id, ModerationAction::Restore))
        }
        _ => None,
    }
}
