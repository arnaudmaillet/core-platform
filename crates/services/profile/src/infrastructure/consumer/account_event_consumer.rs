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
/// The task runs indefinitely; it is expected to be spawned as a background task.
pub async fn run_account_event_consumer<CB: CommandBus>(
    consumer: KafkaConsumerHandle,
    command_bus: CB,
) {
    info!("account event consumer started");
    let mut stream = consumer.stream::<AccountEvent>();

    while let Some(result) = stream.next().await {
        let envelope = match result {
            Ok(e) => e,
            Err(err) => {
                error!(error = ?err, "failed to receive Kafka message; skipping");
                continue;
            }
        };

        let event = &envelope.payload;
        let correlation_id = Uuid::now_v7();

        match event.kind.as_str() {
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
                if let Err(err) = command_bus
                    .dispatch(Envelope::new(correlation_id, cmd))
                    .await
                {
                    warn!(
                        account_id = %event.account_id,
                        event_kind = %event.kind,
                        error = ?err,
                        "hide profile command failed"
                    );
                }
            }

            "AccountActivated" | "AccountReactivated" => {
                let cmd = RestoreProfileCommand {
                    profile_id: event.account_id.clone(),
                };
                if let Err(err) = command_bus
                    .dispatch(Envelope::new(correlation_id, cmd))
                    .await
                {
                    warn!(
                        account_id = %event.account_id,
                        event_kind = %event.kind,
                        error = ?err,
                        "restore profile command failed"
                    );
                }
            }

            other => {
                tracing::trace!(event_kind = other, "ignoring unknown account event kind");
            }
        }
    }

    warn!("account event consumer stream ended unexpectedly");
}
