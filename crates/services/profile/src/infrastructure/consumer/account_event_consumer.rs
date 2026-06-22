use futures::StreamExt as _;
use serde::Deserialize;
use tracing::{error, info, warn};
use uuid::Uuid;

use cqrs::{CommandBus, Envelope};
use transport::kafka::consumer::KafkaConsumerHandle;

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

/// Starts the account event consumer loop.
///
/// Subscribes to the `account.v1.events` topic and translates account lifecycle
/// events into profile masking/restoration commands dispatched through the bus.
///
/// The function is generic over `CB` because `CommandBus` is not object-safe
/// (its `dispatch` method is generic over `C: Command`).
///
/// # Offset management
///
/// This is the equivalent of one `run_once` cycle and assumes the supplied
/// handle was built with `enable_auto_commit = false`. It commits each offset
/// only after the corresponding command has been applied (or the event was
/// intentionally ignored). On a transient dispatch failure it stops *without*
/// committing so the message is redelivered — the supervising task is expected
/// to respawn the consumer, which resumes from the last committed offset.
pub async fn run_account_event_consumer<CB: CommandBus>(
    consumer: KafkaConsumerHandle,
    command_bus: CB,
) {
    info!("account event consumer started");
    let mut stream = consumer.stream::<AccountEvent>();

    while let Some(result) = stream.next().await {
        let msg = match result {
            Ok(m) => m,
            Err(err) => {
                error!(error = ?err, "broker stream error — stopping consumer for respawn");
                return;
            }
        };

        let event = match &msg.payload {
            Ok(e) => e,
            Err(err) => {
                // Poison record — commit past it so it does not block the partition.
                warn!(offset = msg.offset, error = ?err, "deserialization error — committing past poison message");
                if let Err(e) = consumer.commit(&msg) {
                    error!(error = ?e, "commit failed — stopping consumer for respawn");
                    return;
                }
                continue;
            }
        };

        let correlation_id = Uuid::now_v7();

        let dispatch_result = match event.kind.as_str() {
            "AccountSuspended" | "AccountDeleted" => {
                let masking_reason = if event.kind == "AccountDeleted" {
                    "account_deleted"
                } else {
                    "account_suspended"
                };

                // NOTE: HideProfileCommand targets a single profile_id.
                // At the composition root, iterate all profiles for account_id
                // and dispatch one command per profile.
                let cmd = HideProfileCommand {
                    profile_id:        event.account_id.clone(),
                    masking_reason:    masking_reason.to_owned(),
                    suspension_reason: event.reason.clone(),
                };
                command_bus.dispatch(Envelope::new(correlation_id, cmd)).await.err()
            }

            "AccountActivated" | "AccountReactivated" => {
                let cmd = RestoreProfileCommand {
                    profile_id: event.account_id.clone(),
                };
                command_bus.dispatch(Envelope::new(correlation_id, cmd)).await.err()
            }

            other => {
                tracing::trace!(event_kind = other, "ignoring unknown account event kind");
                None
            }
        };

        if let Some(err) = dispatch_result {
            // Transient failure: do NOT commit, so the event is redelivered.
            warn!(
                account_id = %event.account_id,
                event_kind = %event.kind,
                error = ?err,
                "command dispatch failed — offset NOT committed; stopping consumer for respawn"
            );
            return;
        }

        if let Err(e) = consumer.commit(&msg) {
            error!(error = ?e, "commit failed — stopping consumer for respawn");
            return;
        }
    }

    warn!("account event consumer stream ended unexpectedly");
}
