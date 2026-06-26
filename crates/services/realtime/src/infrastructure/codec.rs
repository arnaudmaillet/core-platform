//! The pure proto mapping between the domain / [`DeliverableEvent`] and the
//! generated `realtime.v1` wire types — the one place that knows the `realtime-api`
//! representation, so the rest of the service stays proto-free.
//!
//! Three directions live here:
//! * **outbound to the client** — build a `ServerFrame` (an [`event_frame`],
//!   [`ping_frame`], or [`control_frame`]) from domain values.
//! * **inbound from the client** — decode a `ClientFrame` into a pure
//!   [`ClientIntent`] the gateway dispatches against the domain.
//! * **node hop** — map [`DeliverableEvent`] ⇄ `DeliverEnvelope` (the bytes the
//!   dispatcher publishes and the gateway receives).
//!
//! Everything is total and unit-tested; there is no I/O here.

use chrono::{DateTime, Utc};
use realtime_api as pb;

use crate::application::DeliverableEvent;
use crate::domain::{ChannelClass, ChannelKey, ChannelRef, DeviceId, StreamSeq, UserId};
use crate::error::RealtimeError;

// ── time ──────────────────────────────────────────────────────────────────────

pub(crate) fn epoch() -> DateTime<Utc> {
    DateTime::from_timestamp(0, 0).expect("unix epoch is a valid timestamp")
}

fn ts_to_pb(dt: DateTime<Utc>) -> prost_types::Timestamp {
    prost_types::Timestamp {
        seconds: dt.timestamp(),
        nanos: dt.timestamp_subsec_nanos() as i32,
    }
}

fn ts_from_pb(ts: &prost_types::Timestamp) -> DateTime<Utc> {
    DateTime::from_timestamp(ts.seconds, ts.nanos.max(0) as u32).unwrap_or_else(epoch)
}

// ── channel class / ref ───────────────────────────────────────────────────────

pub fn channel_class_to_pb(class: ChannelClass) -> pb::ChannelClass {
    match class {
        ChannelClass::Dm => pb::ChannelClass::Dm,
        ChannelClass::Notification => pb::ChannelClass::Notification,
        ChannelClass::Presence => pb::ChannelClass::Presence,
        ChannelClass::Counter => pb::ChannelClass::Counter,
        ChannelClass::Feed => pb::ChannelClass::Feed,
    }
}

pub fn channel_class_from_pb(value: i32) -> Result<ChannelClass, RealtimeError> {
    match pb::ChannelClass::try_from(value) {
        Ok(pb::ChannelClass::Dm) => Ok(ChannelClass::Dm),
        Ok(pb::ChannelClass::Notification) => Ok(ChannelClass::Notification),
        Ok(pb::ChannelClass::Presence) => Ok(ChannelClass::Presence),
        Ok(pb::ChannelClass::Counter) => Ok(ChannelClass::Counter),
        Ok(pb::ChannelClass::Feed) => Ok(ChannelClass::Feed),
        Ok(pb::ChannelClass::Unspecified) | Err(_) => Err(RealtimeError::MalformedFrame {
            reason: format!("unknown channel class {value}"),
        }),
    }
}

pub fn channel_to_pb(channel: &ChannelRef) -> pb::ChannelRef {
    pb::ChannelRef {
        class: channel_class_to_pb(channel.class) as i32,
        key: channel.key.as_str().to_owned(),
    }
}

pub fn channel_from_pb(channel: &pb::ChannelRef) -> Result<ChannelRef, RealtimeError> {
    Ok(ChannelRef::new(
        channel_class_from_pb(channel.class)?,
        ChannelKey::new(channel.key.clone())?,
    ))
}

fn channels_from_pb(channels: &[pb::ChannelRef]) -> Result<Vec<ChannelRef>, RealtimeError> {
    channels.iter().map(channel_from_pb).collect()
}

// ── outbound: ServerFrame builders ────────────────────────────────────────────

/// Build the delivery frame for one event on a channel, stamped with the
/// per-`(connection, channel)` sequence the gateway assigned.
pub fn event_frame(
    channel: &ChannelRef,
    stream_seq: StreamSeq,
    event: &DeliverableEvent,
) -> pb::ServerFrame {
    pb::ServerFrame {
        body: Some(pb::server_frame::Body::Event(pb::Event {
            channel: Some(channel_to_pb(channel)),
            stream_seq: stream_seq.get(),
            ack_required: channel.class.ack_required(),
            payload: event.payload.clone(),
            event_type: event.event_type.clone(),
            emitted_at: Some(ts_to_pb(event.emitted_at)),
        })),
    }
}

pub fn ping_frame(nonce: u64) -> pb::ServerFrame {
    pb::ServerFrame {
        body: Some(pb::server_frame::Body::Ping(pb::Ping { nonce })),
    }
}

/// Build a control frame. `channel` is set for SUBSCRIBED / UNSUBSCRIBED; `code`
/// carries the `RTM-XXXX` for ERROR / RATE_LIMITED; `reconnect_after_ms` the base
/// backoff for RECONNECT (the client adds jitter).
pub fn control_frame(
    control: pb::ServerControl,
    channel: Option<&ChannelRef>,
    code: &str,
    message: &str,
    reconnect_after_ms: u32,
) -> pb::ServerFrame {
    pb::ServerFrame {
        body: Some(pb::server_frame::Body::Control(pb::Control {
            control: control as i32,
            channel: channel.map(channel_to_pb),
            code: code.to_owned(),
            message: message.to_owned(),
            reconnect_after_ms,
        })),
    }
}

// ── inbound: ClientFrame → ClientIntent ───────────────────────────────────────

/// A decoded client request, free of any proto type. The gateway's per-socket
/// task dispatches these against its [`crate::domain::Connection`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientIntent {
    Subscribe(Vec<ChannelRef>),
    Unsubscribe(Vec<ChannelRef>),
    Ack {
        channel: ChannelRef,
        stream_seq: StreamSeq,
    },
    Pong {
        nonce: u64,
    },
    AuthRefresh {
        edge_token: String,
    },
    Resume(Vec<(ChannelRef, u64)>),
}

pub fn decode_client_frame(frame: &pb::ClientFrame) -> Result<ClientIntent, RealtimeError> {
    let body = frame
        .body
        .as_ref()
        .ok_or_else(|| RealtimeError::MalformedFrame {
            reason: "client frame has no body".to_owned(),
        })?;

    let intent = match body {
        pb::client_frame::Body::Subscribe(s) => {
            ClientIntent::Subscribe(channels_from_pb(&s.channels)?)
        }
        pb::client_frame::Body::Unsubscribe(u) => {
            ClientIntent::Unsubscribe(channels_from_pb(&u.channels)?)
        }
        pb::client_frame::Body::Ack(a) => ClientIntent::Ack {
            channel: channel_from_pb(require(&a.channel, "ack.channel")?)?,
            stream_seq: StreamSeq::new(a.stream_seq),
        },
        pb::client_frame::Body::Pong(p) => ClientIntent::Pong { nonce: p.nonce },
        pb::client_frame::Body::AuthRefresh(r) => ClientIntent::AuthRefresh {
            edge_token: r.edge_token.clone(),
        },
        pb::client_frame::Body::Resume(r) => {
            let mut cursors = Vec::with_capacity(r.cursors.len());
            for cursor in &r.cursors {
                cursors.push((
                    channel_from_pb(require(&cursor.channel, "resume.channel")?)?,
                    cursor.stream_seq,
                ));
            }
            ClientIntent::Resume(cursors)
        }
    };
    Ok(intent)
}

fn require<'a, T>(field: &'a Option<T>, name: &str) -> Result<&'a T, RealtimeError> {
    field.as_ref().ok_or_else(|| RealtimeError::MalformedFrame {
        reason: format!("missing required field '{name}'"),
    })
}

// ── node hop: DeliverableEvent ⇄ DeliverEnvelope ──────────────────────────────

pub fn envelope_to_pb(event: &DeliverableEvent) -> pb::DeliverEnvelope {
    pb::DeliverEnvelope {
        recipient_user_id: event
            .recipient
            .as_ref()
            .map(|u| u.as_str().to_owned())
            .unwrap_or_default(),
        recipient_device_id: event
            .device_id
            .as_ref()
            .map(|d| d.as_str().to_owned())
            .unwrap_or_default(),
        channel: Some(channel_to_pb(&event.channel)),
        ack_required: event.channel.class.ack_required(),
        payload: event.payload.clone(),
        event_type: event.event_type.clone(),
        emitted_at: Some(ts_to_pb(event.emitted_at)),
        idempotency_key: event.idempotency_key.clone(),
        broadcast: event.is_broadcast(),
    }
}

pub fn envelope_from_pb(env: &pb::DeliverEnvelope) -> Result<DeliverableEvent, RealtimeError> {
    let channel = channel_from_pb(require(&env.channel, "envelope.channel")?)?;
    let device_id = if env.recipient_device_id.is_empty() {
        None
    } else {
        Some(DeviceId::new(env.recipient_device_id.clone())?)
    };
    // A broadcast (or an empty recipient) has no targeted user.
    let recipient = if env.broadcast || env.recipient_user_id.is_empty() {
        None
    } else {
        Some(UserId::new(env.recipient_user_id.clone())?)
    };
    Ok(DeliverableEvent {
        recipient,
        device_id,
        channel,
        payload: env.payload.clone(),
        event_type: env.event_type.clone(),
        emitted_at: env
            .emitted_at
            .as_ref()
            .map(ts_from_pb)
            .unwrap_or_else(epoch),
        idempotency_key: env.idempotency_key.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_event() -> DeliverableEvent {
        DeliverableEvent {
            recipient: Some(UserId::new("alice").unwrap()),
            device_id: Some(DeviceId::new("phone").unwrap()),
            channel: ChannelRef::new(ChannelClass::Dm, ChannelKey::new("alice").unwrap()),
            payload: b"ciphertext".to_vec(),
            event_type: "chat.message".to_owned(),
            emitted_at: DateTime::from_timestamp(1_750_000_000, 0).unwrap(),
            idempotency_key: "evt-1".to_owned(),
        }
    }

    fn broadcast_event() -> DeliverableEvent {
        DeliverableEvent {
            recipient: None,
            device_id: None,
            channel: ChannelRef::new(ChannelClass::Counter, ChannelKey::new("post-42").unwrap()),
            payload: b"{\"score\":9.0}".to_vec(),
            event_type: "counter.popularity".to_owned(),
            emitted_at: DateTime::from_timestamp(1_750_000_000, 0).unwrap(),
            idempotency_key: "pop-1".to_owned(),
        }
    }

    #[test]
    fn channel_class_round_trips_and_rejects_unspecified() {
        for class in [
            ChannelClass::Dm,
            ChannelClass::Notification,
            ChannelClass::Presence,
            ChannelClass::Counter,
            ChannelClass::Feed,
        ] {
            let i = channel_class_to_pb(class) as i32;
            assert_eq!(channel_class_from_pb(i).unwrap(), class);
        }
        assert!(channel_class_from_pb(0).is_err()); // UNSPECIFIED
        assert!(channel_class_from_pb(99).is_err()); // out of range
    }

    #[test]
    fn deliver_envelope_round_trips() {
        let event = sample_event();
        let pb = envelope_to_pb(&event);
        assert!(pb.ack_required); // DM ⇒ at-least-once
        assert!(!pb.broadcast); // targeted
        let back = envelope_from_pb(&pb).unwrap();
        assert_eq!(back, event);
    }

    #[test]
    fn broadcast_envelope_round_trips_with_no_recipient() {
        let event = broadcast_event();
        let pb = envelope_to_pb(&event);
        assert!(pb.broadcast);
        assert!(pb.recipient_user_id.is_empty());
        let back = envelope_from_pb(&pb).unwrap();
        assert!(back.is_broadcast());
        assert_eq!(back, event);
    }

    #[test]
    fn envelope_without_device_maps_to_none() {
        let mut event = sample_event();
        event.device_id = None;
        let pb = envelope_to_pb(&event);
        assert!(pb.recipient_device_id.is_empty());
        assert_eq!(envelope_from_pb(&pb).unwrap().device_id, None);
    }

    #[test]
    fn event_frame_carries_sequence_and_opaque_payload() {
        let event = sample_event();
        let frame = event_frame(&event.channel, StreamSeq::new(7), &event);
        match frame.body.unwrap() {
            pb::server_frame::Body::Event(e) => {
                assert_eq!(e.stream_seq, 7);
                assert!(e.ack_required);
                assert_eq!(e.payload, b"ciphertext");
                assert_eq!(e.event_type, "chat.message");
            }
            _ => panic!("expected an Event frame"),
        }
    }

    #[test]
    fn decodes_a_subscribe_frame() {
        let frame = pb::ClientFrame {
            body: Some(pb::client_frame::Body::Subscribe(pb::Subscribe {
                channels: vec![pb::ChannelRef {
                    class: pb::ChannelClass::Dm as i32,
                    key: "alice".to_owned(),
                }],
            })),
        };
        match decode_client_frame(&frame).unwrap() {
            ClientIntent::Subscribe(chs) => {
                assert_eq!(chs.len(), 1);
                assert_eq!(chs[0].to_string(), "dm:alice");
            }
            other => panic!("expected Subscribe, got {other:?}"),
        }
    }

    #[test]
    fn decodes_an_ack_frame() {
        let frame = pb::ClientFrame {
            body: Some(pb::client_frame::Body::Ack(pb::ClientAck {
                channel: Some(pb::ChannelRef {
                    class: pb::ChannelClass::Notification as i32,
                    key: "alice".to_owned(),
                }),
                stream_seq: 42,
            })),
        };
        match decode_client_frame(&frame).unwrap() {
            ClientIntent::Ack { stream_seq, .. } => assert_eq!(stream_seq.get(), 42),
            other => panic!("expected Ack, got {other:?}"),
        }
    }

    #[test]
    fn empty_client_frame_is_malformed() {
        let err = decode_client_frame(&pb::ClientFrame { body: None }).unwrap_err();
        assert_eq!(<RealtimeError as error::AppError>::error_code(&err), "RTM-2001");
    }
}
