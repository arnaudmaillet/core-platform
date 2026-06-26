use std::sync::Arc;

use serde::Deserialize;
use tracing::{error, info};

use error::AppError;
use transport::kafka::consumer::{run_consumer, KafkaConsumerHandle, ProcessOutcome, RetryPolicy};
use transport::kafka::producer::KafkaProducerHandle;

use crate::application::port::AuthorTierStore;
use crate::domain::value_object::ProfileId;

/// Lenient read DTO for `profile.v1.events` (the internally-tagged
/// `{"type": ...}` stream). Only `ProfileTierChanged` is acted on; all other
/// variants deserialize and are skipped.
#[derive(Debug, Deserialize)]
struct ProfileV1Event {
    #[serde(rename = "type")]
    event_type: String,
    #[serde(default)]
    profile_id: String,
    #[serde(default)]
    tier: u8,
}

/// Runs the author-tier projection consumer on the shared at-least-once runner.
///
/// Consumes `profile.v1.events`, upserts the `profile_id → tier` projection on each
/// `ProfileTierChanged`, and commits everything else as a no-op. The upsert is
/// last-writer-wins and idempotent, so at-least-once redelivery is harmless. The
/// publish path reads this projection to stamp `author_tier` onto published posts.
pub async fn run_author_tier_consumer(
    consumer: KafkaConsumerHandle,
    store: Arc<dyn AuthorTierStore>,
    producer: KafkaProducerHandle,
) {
    info!("post author-tier consumer started");

    let policy = RetryPolicy::default();
    let result = run_consumer::<ProfileV1Event, _>(&consumer, &producer, &policy, move |event| {
        let store = Arc::clone(&store);
        Box::pin(async move { process_event(store.as_ref(), event).await })
    })
    .await;

    if let Err(e) = result {
        error!(error = %e, "post author-tier consumer stopped");
    }
}

async fn process_event(store: &dyn AuthorTierStore, event: &ProfileV1Event) -> ProcessOutcome {
    if event.event_type != "ProfileTierChanged" {
        return ProcessOutcome::Done; // not a tier event — commit and skip
    }

    let profile_id = match ProfileId::try_from(event.profile_id.as_str()) {
        Ok(id) => id,
        Err(e) => return ProcessOutcome::Reject(e.to_string()),
    };

    match store.upsert_tier(&profile_id, event.tier).await {
        Ok(())                     => ProcessOutcome::Done,
        Err(e) if e.is_retryable() => ProcessOutcome::Retry(e.to_string()),
        Err(e)                     => ProcessOutcome::Reject(e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_tier_change_event() {
        let json = r#"{"type":"ProfileTierChanged","profile_id":"prof-9","tier":2,"occurred_at_ms":1}"#;
        let ev: ProfileV1Event = serde_json::from_str(json).unwrap();
        assert_eq!(ev.event_type, "ProfileTierChanged");
        assert_eq!(ev.profile_id, "prof-9");
        assert_eq!(ev.tier, 2);
    }

    #[test]
    fn other_profile_events_are_not_tier_changes() {
        // A non-tier event still deserializes (tier defaults to 0) but is skipped
        // by the `event_type` guard in `process_event`.
        for json in [
            r#"{"type":"ProfileUpdated","profile_id":"prof-9","occurred_at_ms":1}"#,
            r#"{"type":"ProfileVerified","profile_id":"prof-9","occurred_at_ms":1}"#,
        ] {
            let ev: ProfileV1Event = serde_json::from_str(json).unwrap();
            assert_ne!(ev.event_type, "ProfileTierChanged");
            assert_eq!(ev.tier, 0);
        }
    }
}
