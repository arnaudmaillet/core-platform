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

use chrono::{DateTime, Utc};
use serde::Deserialize;

use crate::application::DeliverableEvent;
use crate::domain::{ChannelClass, ChannelKey, ChannelRef, DeviceId, UserId};
use crate::error::RealtimeError;
use crate::infrastructure::codec::epoch;

const TOPIC_NOTIFICATION: &str = "notification.v1.events";
const TOPIC_POPULARITY: &str = "counter.v1.popularity";
const TOPIC_POST: &str = "post.v1.events";

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

/// Map a `notification.v1.events` record to a **targeted** deliverable event on the
/// recipient's `notif` channel (identity-scoped, at-least-once). `Ok(None)` would
/// signal a harmless skip; a notification always has a recipient, so this is always
/// `Some`.
pub fn map_notification(
    wire: NotificationWire,
) -> Result<Option<DeliverableEvent>, RealtimeError> {
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

    Ok(Some(DeliverableEvent {
        recipient: Some(recipient),
        device_id,
        channel,
        payload,
        event_type: format!("notif.{}", wire.kind),
        emitted_at: DateTime::from_timestamp_millis(wire.created_at_ms).unwrap_or_else(epoch),
        idempotency_key: wire.notification_id,
    }))
}

// ── counter.v1.popularity → public broadcast on counter:<entity> ──────────────

/// The coarse popularity signal `counter` publishes — entity-addressed, no
/// recipient. `score` and `entity_type` ride along as the client-facing payload.
#[derive(Debug, Clone, Deserialize)]
pub struct PopularityWire {
    pub entity_type: String,
    pub entity_id: String,
    pub score: f64,
}

/// Map a popularity signal to a **public broadcast** on `counter:<entity_id>` —
/// delivered to every connection viewing that entity. Fire-and-forget (latest
/// wins). An empty entity id is unroutable and skipped (`Ok(None)`).
pub fn map_counter_popularity(
    wire: PopularityWire,
) -> Result<Option<DeliverableEvent>, RealtimeError> {
    if wire.entity_id.trim().is_empty() {
        return Ok(None);
    }
    let channel = ChannelRef::new(
        ChannelClass::Counter,
        ChannelKey::new(wire.entity_id.clone())?,
    );
    let payload = serde_json::to_vec(&serde_json::json!({
        "entity_type": wire.entity_type,
        "entity_id": wire.entity_id,
        "score": wire.score,
    }))
    .map_err(|e| RealtimeError::EventDecodeFailed {
        topic: TOPIC_POPULARITY.to_owned(),
        reason: e.to_string(),
    })?;

    Ok(Some(DeliverableEvent {
        recipient: None,
        device_id: None,
        channel,
        payload,
        event_type: "counter.popularity".to_owned(),
        emitted_at: Utc::now(),
        idempotency_key: format!("pop:{}:{}", wire.entity_type, wire.entity_id),
    }))
}

// ── post.v1.events → public broadcast on feed:<author> ────────────────────────

/// A `post.v1.events` lifecycle record (`{"type": "...", post_id, profile_id}`).
/// Lenient: extra fields ignored.
#[derive(Debug, Clone, Deserialize)]
pub struct PostWire {
    #[serde(rename = "type", default)]
    pub event_type: String,
    #[serde(default)]
    pub post_id: String,
    /// The author — the `feed:<profile_id>` channel its viewers subscribe to.
    #[serde(default)]
    pub profile_id: String,
}

/// Map a post lifecycle event to a **public broadcast** on `feed:<profile_id>` —
/// the author's feed channel. Missing an author is unroutable and skipped.
pub fn map_post(wire: PostWire) -> Result<Option<DeliverableEvent>, RealtimeError> {
    if wire.profile_id.trim().is_empty() {
        return Ok(None);
    }
    let channel = ChannelRef::new(ChannelClass::Feed, ChannelKey::new(wire.profile_id.clone())?);
    let payload = serde_json::to_vec(&serde_json::json!({
        "type": wire.event_type,
        "post_id": wire.post_id,
        "author_id": wire.profile_id,
    }))
    .map_err(|e| RealtimeError::EventDecodeFailed {
        topic: TOPIC_POST.to_owned(),
        reason: e.to_string(),
    })?;

    Ok(Some(DeliverableEvent {
        recipient: None,
        device_id: None,
        channel,
        payload,
        event_type: format!("post.{}", wire.event_type),
        emitted_at: Utc::now(),
        idempotency_key: format!("post:{}:{}", wire.event_type, wire.post_id),
    }))
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
        let ev = map_notification(wire).unwrap().unwrap();
        assert_eq!(ev.recipient.unwrap().as_str(), "alice");
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

    #[test]
    fn maps_popularity_to_a_public_counter_broadcast() {
        let wire = PopularityWire {
            entity_type: "post".to_owned(),
            entity_id: "post-42".to_owned(),
            score: 9.5,
        };
        let ev = map_counter_popularity(wire).unwrap().unwrap();
        assert!(ev.is_broadcast()); // no recipient
        assert_eq!(ev.channel.to_string(), "counter:post-42");
        assert_eq!(ev.event_type, "counter.popularity");
    }

    #[test]
    fn maps_post_event_to_a_public_feed_broadcast() {
        let wire = PostWire {
            event_type: "PostPublished".to_owned(),
            post_id: "p-1".to_owned(),
            profile_id: "author-7".to_owned(),
        };
        let ev = map_post(wire).unwrap().unwrap();
        assert!(ev.is_broadcast());
        assert_eq!(ev.channel.to_string(), "feed:author-7");
        assert_eq!(ev.event_type, "post.PostPublished");
    }

    #[test]
    fn unroutable_public_events_are_skipped() {
        // No entity id / no author ⇒ Ok(None), a harmless commit (not a DLQ).
        let pop = PopularityWire {
            entity_type: "post".to_owned(),
            entity_id: "  ".to_owned(),
            score: 1.0,
        };
        assert!(map_counter_popularity(pop).unwrap().is_none());

        let post = PostWire {
            event_type: "PostPublished".to_owned(),
            post_id: "p-1".to_owned(),
            profile_id: String::new(),
        };
        assert!(map_post(post).unwrap().is_none());
    }
}
