//! The dispatcher's inbound decode layer: realtime-owned wire DTOs for the
//! upstream streams it fans out, plus the pure `map_*` distillation into a
//! [`DeliverableEvent`].
//!
//! Like `counter` / `search`, realtime must not depend on the producing services'
//! crates (a sideways services→services edge the tiering forbids), so it owns its
//! read schema: minimal, lenient structs that match the published JSON (extra
//! fields ignored, so an additive upstream change never breaks the consumer).
//!
//! Integration reality (honest about upstream readiness): only the
//! `notification.v1.events` mapping is concrete here — `notification` publishes a
//! per-recipient stream today. The `chat` / `counter.v1.popularity` / `post.v1`
//! mappings follow the **same shape** (decode → address a recipient + channel →
//! carry an opaque payload) and land as those producers are wired; they are an
//! upstream prerequisite, not a gap in this layer. The realtime plane forwards
//! `payload` verbatim and never interprets it.

use chrono::DateTime;
use serde::Deserialize;

use crate::application::DeliverableEvent;
use crate::domain::{ChannelClass, ChannelKey, ChannelRef, DeviceId, UserId};
use crate::error::RealtimeError;
use crate::infrastructure::codec::epoch;

const TOPIC_NOTIFICATION: &str = "notification.v1.events";

/// A notification destined for one recipient. `payload` is the already-rendered
/// client view (badge counts / preview) the plane forwards verbatim.
#[derive(Debug, Clone, Deserialize)]
pub struct NotificationWire {
    pub recipient_id: String,
    /// Target a single device; absent ⇒ all of the recipient's connections.
    #[serde(default)]
    pub device_id: Option<String>,
    /// The notification's stable id — the delivery idempotency key.
    pub notification_id: String,
    pub kind: String,
    pub created_at_ms: i64,
    /// Opaque, client-ready payload. Forwarded as bytes; never interpreted.
    #[serde(default)]
    pub payload: serde_json::Value,
}

/// Map a `notification.v1.events` record to a deliverable event on the recipient's
/// `notif` channel (identity-scoped, at-least-once).
pub fn map_notification(wire: NotificationWire) -> Result<DeliverableEvent, RealtimeError> {
    let recipient = UserId::new(wire.recipient_id)?;
    let channel = ChannelRef::new(
        ChannelClass::Notification,
        ChannelKey::new(recipient.as_str().to_owned())?,
    );
    let payload =
        serde_json::to_vec(&wire.payload).map_err(|e| RealtimeError::EventDecodeFailed {
            topic: TOPIC_NOTIFICATION.to_owned(),
            reason: e.to_string(),
        })?;
    let device_id = wire.device_id.map(DeviceId::new).transpose()?;

    Ok(DeliverableEvent {
        recipient,
        device_id,
        channel,
        payload,
        event_type: format!("notif.{}", wire.kind),
        emitted_at: DateTime::from_timestamp_millis(wire.created_at_ms).unwrap_or_else(epoch),
        idempotency_key: wire.notification_id,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_a_notification_onto_the_recipients_notif_channel() {
        let wire = NotificationWire {
            recipient_id: "alice".to_owned(),
            device_id: None,
            notification_id: "ntf-9".to_owned(),
            kind: "follow".to_owned(),
            created_at_ms: 1_750_000_000_000,
            payload: serde_json::json!({ "unread": 3 }),
        };
        let ev = map_notification(wire).unwrap();
        assert_eq!(ev.recipient.as_str(), "alice");
        // Identity-scoped: the channel key is the recipient itself.
        assert_eq!(ev.channel.to_string(), "notif:alice");
        assert_eq!(ev.event_type, "notif.follow");
        assert_eq!(ev.idempotency_key, "ntf-9");
        assert_eq!(ev.device_id, None);
    }

    #[test]
    fn rejects_a_blank_recipient() {
        let wire = NotificationWire {
            recipient_id: "  ".to_owned(),
            device_id: None,
            notification_id: "ntf-1".to_owned(),
            kind: "x".to_owned(),
            created_at_ms: 0,
            payload: serde_json::Value::Null,
        };
        let err = map_notification(wire).unwrap_err();
        assert_eq!(<RealtimeError as error::AppError>::error_code(&err), "RTM-9002");
    }
}
