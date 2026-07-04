use std::sync::Arc;

use serde::Deserialize;
use tracing::{error, info};
use uuid::Uuid;

use cqrs::{CommandBus, Envelope};
use error::AppError;
use transport::kafka::consumer::{
    run_consumer, KafkaConsumerHandle, ProcessOutcome, RetryPolicy,
};
use transport::kafka::producer::KafkaProducerHandle;

use crate::application::command::{HideProfileCommand, RestoreProfileCommand};

/// Kafka event payload published by the account service on `account.v1.events`.
///
/// Only the fields relevant to profile masking/restoration are deserialized.
/// Unknown event kinds are silently ignored.
#[derive(Debug, Deserialize)]
struct AccountEvent {
    #[serde(rename = "event_kind")]
    kind: String,
    account_id: String,
    #[serde(default)]
    reason: Option<String>,
}

/// Runs the account event consumer on the shared at-least-once runner.
///
/// Subscribes (via the supplied handle) to `account.v1.events` and translates
/// account lifecycle events into profile masking/restoration commands. The runner
/// owns the decode → process → retry → dead-letter → commit loop: transient
/// dispatch failures are retried with backoff then dead-lettered, poison records
/// are dead-lettered immediately, and unknown event kinds are committed as no-ops.
///
/// The function is generic over `CB` because `CommandBus` is not object-safe
/// (its `dispatch` method is generic over `C: Command`). The handle must be built
/// with `enable_auto_commit = false`. Returns when the stream ends or on an
/// unrecoverable broker/dead-letter error; the supervising task should respawn it.
pub async fn run_account_event_consumer<CB: CommandBus + 'static>(
    consumer: KafkaConsumerHandle,
    command_bus: CB,
    producer: KafkaProducerHandle,
) {
    info!("account event consumer started");

    // `Arc<CB>` so the per-message closure captures an owned handle (the returned
    // futures then borrow only the event, satisfying the runner bound).
    let command_bus = Arc::new(command_bus);
    let policy = RetryPolicy::default();

    let result = run_consumer::<AccountEvent, _>(&consumer, &producer, &policy, move |event| {
        let command_bus = Arc::clone(&command_bus);
        Box::pin(async move { process_event(command_bus.as_ref(), event).await })
    })
    .await;

    if let Err(e) = result {
        error!(error = %e, "account event consumer stopped");
    }
}

/// Translates one account event into the matching profile command and classifies
/// the result. Unknown event kinds are intentional no-ops (`Done`, so they commit
/// rather than dead-letter); a transient dispatch failure is retried then
/// dead-lettered, and a permanent one is dead-lettered immediately.
async fn process_event<CB: CommandBus>(command_bus: &CB, event: &AccountEvent) -> ProcessOutcome {
    let correlation_id = Uuid::now_v7();

    let dispatch = match event.kind.as_str() {
        "AccountSuspended" | "AccountDeleted" => {
            let masking_reason = if event.kind == "AccountDeleted" {
                "account_deleted"
            } else {
                "account_suspended"
            };

            // NOTE: HideProfileCommand targets a single profile_id. At the
            // composition root, iterate all profiles for account_id and dispatch
            // one command per profile.
            let cmd = HideProfileCommand {
                profile_id:        event.account_id.clone(),
                masking_reason:    masking_reason.to_owned(),
                suspension_reason: event.reason.clone(),
            };
            command_bus.dispatch(Envelope::new(correlation_id, cmd)).await
        }

        "AccountActivated" | "AccountReactivated" => {
            let cmd = RestoreProfileCommand {
                profile_id: event.account_id.clone(),
            };
            command_bus.dispatch(Envelope::new(correlation_id, cmd)).await
        }

        other => {
            tracing::trace!(event_kind = other, "ignoring unknown account event kind");
            return ProcessOutcome::Done;
        }
    };

    match dispatch {
        Ok(())                     => ProcessOutcome::Done,
        Err(e) if e.is_retryable() => ProcessOutcome::Retry(e.to_string()),
        Err(e)                     => ProcessOutcome::Reject(e.to_string()),
    }
}
