use std::sync::Arc;

use chrono::Utc;
use cqrs::Envelope;
use tracing::{error, info};
use transport::kafka::consumer::{KafkaConsumerHandle, ProcessOutcome, RetryPolicy, run_consumer};
use transport::kafka::producer::KafkaProducerHandle;
use uuid::Uuid;

use crate::application::command::ProjectionHandler;
use crate::infrastructure::decode::{Decoded, PostWireEvent, map_post};
use crate::infrastructure::hydrate::SourceHydrator;

/// Runs the `post.v1.events` consumer on the shared at-least-once runner. A
/// content notification is hydrated (gRPC) into a full snapshot before projection;
/// a delete projects directly. `run_consumer` owns deserialization, so a poison
/// message dead-letters before it ever reaches us.
pub async fn run_post_consumer(
    consumer: KafkaConsumerHandle,
    projection: Arc<ProjectionHandler>,
    hydrator: Arc<dyn SourceHydrator>,
    producer: KafkaProducerHandle,
) {
    info!("search post consumer started");
    let policy = RetryPolicy::default();
    let result = run_consumer::<PostWireEvent, _>(&consumer, &producer, &policy, move |event| {
        let projection = Arc::clone(&projection);
        let hydrator = Arc::clone(&hydrator);
        Box::pin(async move { process(projection, hydrator, event).await })
    })
    .await;
    if let Err(e) = result {
        error!(error = %e, "search post consumer stopped");
    }
}

async fn process(
    projection: Arc<ProjectionHandler>,
    hydrator: Arc<dyn SourceHydrator>,
    event: &PostWireEvent,
) -> ProcessOutcome {
    let now = Utc::now();
    let result = match map_post(event.clone()) {
        Decoded::Ready(source_event) => projection
            .apply(Envelope::new(Uuid::now_v7(), source_event), now)
            .await
            .map(|_| ()),
        Decoded::NeedsContent(content_ref) => match hydrator.hydrate(content_ref, now).await {
            Ok(source_event) => projection
                .apply(Envelope::new(Uuid::now_v7(), source_event), now)
                .await
                .map(|_| ()),
            Err(e) => Err(e),
        },
        // Nothing to index (shouldn't occur for post events) — commit.
        Decoded::Ignore => Ok(()),
    };
    ProcessOutcome::from_result(result)
}
