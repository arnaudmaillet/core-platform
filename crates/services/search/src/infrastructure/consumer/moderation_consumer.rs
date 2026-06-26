use std::sync::Arc;

use chrono::Utc;
use cqrs::Envelope;
use tracing::{error, info};
use transport::kafka::consumer::{KafkaConsumerHandle, ProcessOutcome, RetryPolicy, run_consumer};
use transport::kafka::producer::KafkaProducerHandle;
use uuid::Uuid;

use crate::application::command::ProjectionHandler;
use crate::infrastructure::decode::{Decoded, ModerationWireEvent, map_moderation};

/// Runs the `moderation.v1.events` consumer. Visibility transitions are projected
/// directly (no hydration); every other moderation event is a benign no-op that
/// still commits the offset.
pub async fn run_moderation_consumer(
    consumer: KafkaConsumerHandle,
    projection: Arc<ProjectionHandler>,
    producer: KafkaProducerHandle,
) {
    info!("search moderation consumer started");
    let policy = RetryPolicy::default();
    let result =
        run_consumer::<ModerationWireEvent, _>(&consumer, &producer, &policy, move |event| {
            let projection = Arc::clone(&projection);
            Box::pin(async move { process(projection, event).await })
        })
        .await;
    if let Err(e) = result {
        error!(error = %e, "search moderation consumer stopped");
    }
}

async fn process(projection: Arc<ProjectionHandler>, event: &ModerationWireEvent) -> ProcessOutcome {
    let result = match map_moderation(event.clone()) {
        Decoded::Ready(source_event) => projection
            .apply(Envelope::new(Uuid::now_v7(), source_event), Utc::now())
            .await
            .map(|_| ()),
        // `NeedsContent` never arises for moderation events; treat as a no-op.
        Decoded::NeedsContent(_) | Decoded::Ignore => Ok(()),
    };
    ProcessOutcome::from_result(result)
}
