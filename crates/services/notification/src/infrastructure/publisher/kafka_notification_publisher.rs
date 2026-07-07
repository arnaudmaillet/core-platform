use async_trait::async_trait;
use serde::Serialize;
use transport::error::TransportError;
use transport::kafka::envelope::EventEnvelope;
use transport::kafka::producer::KafkaProducerHandle;

use crate::application::port::{NotificationEventPublisher, NotificationStreamEvent};
use crate::error::NotificationError;

/// The realtime push stream. `realtime` maps each record onto the recipient's
/// identity-scoped `notif` channel and forwards `payload` verbatim.
const TOPIC: &str = "notification.v1.events";

/// The wire body. Field names MUST match realtime's `NotificationWire` decode
/// struct — this is the (name-and-shape) contract the event-topology registry
/// wires but does not itself enforce. Owned (not borrowed) because the producer
/// requires a `'static` payload.
#[derive(Serialize)]
struct Wire {
    recipient_id:    String,
    notification_id: String,
    kind:            String,
    created_at_ms:   i64,
    payload:         serde_json::Value,
}

/// Publishes `notification.v1.events` over Kafka, keyed by recipient.
pub struct KafkaNotificationPublisher {
    producer: KafkaProducerHandle,
}

impl KafkaNotificationPublisher {
    pub fn new(producer: KafkaProducerHandle) -> Self {
        Self { producer }
    }
}

#[async_trait]
impl NotificationEventPublisher for KafkaNotificationPublisher {
    async fn publish(&self, event: &NotificationStreamEvent) -> Result<(), NotificationError> {
        // Keyed by recipient so one recipient's notifications keep per-partition
        // order (matching realtime's per-identity `notif` channel).
        let envelope = EventEnvelope::new(
            TOPIC,
            event.recipient_id.clone(),
            Wire {
                recipient_id:    event.recipient_id.clone(),
                notification_id: event.notification_id.clone(),
                kind:            event.kind.clone(),
                created_at_ms:   event.created_at_ms,
                payload:         event.payload.clone(),
            },
        )
        .with_header("event_type", format!("notif.{}", event.kind))
        .with_header("recipient_id", event.recipient_id.clone());

        self.producer.publish(envelope).await.map_err(transport_err)
    }
}

fn transport_err(e: TransportError) -> NotificationError {
    NotificationError::EventPublishFailed { message: e.to_string() }
}

/// No-op publisher for the Kafka-less build (integration harness, `Backends.kafka
/// = None`): the command path stays driveable without a broker.
pub struct NoopNotificationPublisher;

#[async_trait]
impl NotificationEventPublisher for NoopNotificationPublisher {
    async fn publish(&self, _event: &NotificationStreamEvent) -> Result<(), NotificationError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The wire body must serialize with exactly the field names realtime's
    /// `NotificationWire` reads. realtime owns its own lenient decode struct (it
    /// can't depend on this crate), so this is the one place the shape is pinned.
    #[test]
    fn wire_matches_realtime_notification_contract() {
        let wire = Wire {
            recipient_id:    "alice".to_owned(),
            notification_id: "ntf-9".to_owned(),
            kind:            "follow".to_owned(),
            created_at_ms:   1_750_000_000_000,
            payload:         serde_json::json!({ "sender_count": 3 }),
        };

        let v = serde_json::to_value(&wire).expect("serialize wire");
        assert_eq!(v["recipient_id"], "alice");
        assert_eq!(v["notification_id"], "ntf-9");
        assert_eq!(v["kind"], "follow");
        assert_eq!(v["created_at_ms"], 1_750_000_000_000_i64);
        assert_eq!(v["payload"]["sender_count"], 3);
        // device_id is omitted (realtime treats absent ⇒ all of the recipient's
        // connections); it must not appear as an unexpected null-typed field.
        assert!(v.get("device_id").is_none());
    }
}
