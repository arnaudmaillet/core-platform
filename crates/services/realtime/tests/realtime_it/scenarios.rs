//! The live bridge scenarios, driven over real Redis.

use std::sync::Arc;
use std::time::Duration;

use fred::interfaces::{EventInterface, PubsubInterface};
use prost::Message as _;
use realtime_api as pb;
use tokio::time::timeout;
use uuid::Uuid;

use realtime::application::port::ConnectionRegistry;
use realtime::domain::ConnectionId;
use realtime::infrastructure::runtime::ConnectionTable;
use test_support::await_until;

use super::harness::*;

/// `HSET`/`HGETALL`/`HDEL` round-trip: two placements bind, resolve, and evict one.
#[tokio::test]
async fn registry_binds_resolves_and_evicts() {
    let h = Harness::start().await;
    let user = fresh_user();

    h.registry
        .bind(&location(&user, "phone", "c1", "node-A"))
        .await
        .unwrap();
    h.registry
        .bind(&location(&user, "tablet", "c2", "node-B"))
        .await
        .unwrap();
    assert_eq!(h.registry.resolve(&user).await.unwrap().len(), 2);

    h.registry
        .evict(&user, &ConnectionId::new("c1").unwrap())
        .await
        .unwrap();
    let remaining = h.registry.resolve(&user).await.unwrap();
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].connection_id.as_str(), "c2");
}

/// A connection whose `evict` never ran (a half-open leak) must be reclaimed by
/// the bind-time TTL — the self-heal that keeps the registry from accreting ghosts.
#[tokio::test]
async fn registry_ttl_self_heals_a_leaked_entry() {
    let h = Harness::with_ttl(SHORT_TTL_MS).await;
    let user = fresh_user();

    h.registry
        .bind(&location(&user, "phone", "c1", "node-A"))
        .await
        .unwrap();
    assert_eq!(h.registry.resolve(&user).await.unwrap().len(), 1);

    await_until("leaked registry entry expires", Duration::from_secs(5), || async {
        h.registry.resolve(&user).await.unwrap().is_empty()
    })
    .await;
}

/// `SPUBLISH`→`SSUBSCRIBE` carries a prost `DeliverEnvelope` intact (the node hop).
#[tokio::test]
async fn node_hop_carries_a_deliver_envelope() {
    let h = Harness::start().await;
    let user = fresh_user();
    let node = format!("node-{}", Uuid::now_v7());

    let subscriber = h.subscriber().await;
    subscriber
        .inner
        .ssubscribe(format!("rt:node:{{{node}}}"))
        .await
        .unwrap();
    let mut rx = subscriber.inner.message_rx();

    publish(&h.node_channel, &node, &dm_event(&user, b"hello-bridge")).await;

    let msg = timeout(Duration::from_secs(5), rx.recv())
        .await
        .expect("node-hop recv timed out")
        .expect("recv");
    let bytes = msg.value.as_bytes().expect("binary payload");
    let env = pb::DeliverEnvelope::decode(bytes).expect("decode envelope");

    assert_eq!(env.recipient_user_id, user.as_str());
    assert_eq!(env.payload, b"hello-bridge");
    assert!(env.ack_required); // DM ⇒ at-least-once
}

/// The full internal→external bridge over real Redis: fan-out resolves the live
/// registry, hops to the owning node, and the table lands an `Event` frame on the
/// subscribed connection's queue.
#[tokio::test]
async fn bridge_delivers_event_to_a_subscribed_connection() {
    let h = Harness::start().await;
    let user = fresh_user();
    let node = format!("node-{}", Uuid::now_v7());

    let table = Arc::new(ConnectionTable::new());
    let mut socket_rx = register_connection(&table, &user, "phone", "c1", &node, 8).await;
    h.registry
        .bind(&location(&user, "phone", "c1", &node))
        .await
        .unwrap();

    // Subscribe the node channel BEFORE fanning out (pub/sub is fire-and-forget).
    let subscriber = h.subscriber().await;
    subscriber
        .inner
        .ssubscribe(format!("rt:node:{{{node}}}"))
        .await
        .unwrap();
    let mut node_rx = subscriber.inner.message_rx();

    let outcome = h
        .fan_out()
        .fan_out(&dm_event(&user, b"payload-xyz"))
        .await
        .unwrap();
    assert!(!outcome.offline);
    assert_eq!(outcome.nodes_published, 1);

    // Receive the envelope off the node channel and deliver it via the table.
    let msg = timeout(Duration::from_secs(5), node_rx.recv())
        .await
        .expect("node recv timed out")
        .expect("recv");
    let env = pb::DeliverEnvelope::decode(msg.value.as_bytes().expect("bytes")).unwrap();
    let event = realtime::infrastructure::codec::envelope_from_pb(&env).unwrap();
    assert_eq!(table.deliver(&event).await, 1);

    // The connection's socket queue received an Event frame for dm:<user>, seq 1.
    let frame_bytes = timeout(Duration::from_secs(5), socket_rx.recv())
        .await
        .expect("socket recv timed out")
        .expect("frame");
    match pb::ServerFrame::decode(&frame_bytes[..]).unwrap().body.unwrap() {
        pb::server_frame::Body::Event(e) => {
            assert_eq!(e.payload, b"payload-xyz");
            assert_eq!(e.stream_seq, 1);
        }
        _ => panic!("expected an Event frame"),
    }
}

/// A recipient with no live placement is a fail-open no-op — nothing is published.
#[tokio::test]
async fn offline_recipient_is_a_noop() {
    let h = Harness::start().await;
    let user = fresh_user(); // never bound

    let outcome = h
        .fan_out()
        .fan_out(&dm_event(&user, b"x"))
        .await
        .unwrap();
    assert!(outcome.offline);
    assert_eq!(outcome.nodes_published, 0);
}
