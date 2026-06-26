use std::sync::Arc;

use serde::Deserialize;
use tracing::{error, info};
use uuid::Uuid;

use cqrs::{CommandBus, Envelope};
use error::AppError;
use transport::kafka::consumer::{run_consumer, KafkaConsumerHandle, ProcessOutcome, RetryPolicy};
use transport::kafka::producer::KafkaProducerHandle;

use crate::application::command::SetProfileTierCommand;

/// Kafka payload on `social-graph.author_tier_changed` (produced by social-graph
/// when a follow/unfollow crosses a follower-count tier boundary).
#[derive(Debug, Deserialize)]
struct AuthorTierChangedEvent {
    profile_id: String,
    /// 0=Standard, 1=Premium, 2=Vip.
    new_tier: u8,
}

/// Runs the author-tier consumer on the shared at-least-once runner.
///
/// Translates each `social-graph.author_tier_changed` event into a
/// [`SetProfileTierCommand`], which persists the tier on the profile and re-emits
/// it on `profile.v1.events` for `post` to denormalize. The runner owns the
/// decode → process → retry → dead-letter → commit loop; the command is idempotent
/// (an unchanged tier is a no-op), so an at-least-once redelivery is harmless. The
/// handle must be built with `enable_auto_commit = false`.
pub async fn run_author_tier_consumer<CB: CommandBus + 'static>(
    consumer: KafkaConsumerHandle,
    command_bus: CB,
    producer: KafkaProducerHandle,
) {
    info!("author tier consumer started");

    let command_bus = Arc::new(command_bus);
    let policy = RetryPolicy::default();

    let result = run_consumer::<AuthorTierChangedEvent, _>(&consumer, &producer, &policy, move |event| {
        let command_bus = Arc::clone(&command_bus);
        Box::pin(async move { process_event(command_bus.as_ref(), event).await })
    })
    .await;

    if let Err(e) = result {
        error!(error = %e, "author tier consumer stopped");
    }
}

async fn process_event<CB: CommandBus>(
    command_bus: &CB,
    event: &AuthorTierChangedEvent,
) -> ProcessOutcome {
    let cmd = SetProfileTierCommand {
        profile_id: event.profile_id.clone(),
        tier: event.new_tier,
    };
    match command_bus.dispatch(Envelope::new(Uuid::now_v7(), cmd)).await {
        Ok(())                     => ProcessOutcome::Done,
        Err(e) if e.is_retryable() => ProcessOutcome::Retry(e.to_string()),
        Err(e)                     => ProcessOutcome::Reject(e.to_string()),
    }
}
