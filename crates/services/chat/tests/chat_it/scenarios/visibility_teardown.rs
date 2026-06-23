//! Scenario 4 — Kafka-driven Audience-Plane teardown on unpublish.
//!
//! Making a conversation private emits `chat.conversation.unpublished` to Kafka;
//! every pod's [`VisibilityWorker`](chat::infrastructure::worker::VisibilityWorker)
//! consumes it and (1) clears the audience-shard routing registry so publishers
//! stop fanning the shadow, and (2) closes the local audience streams — cancelling
//! live guest connections. The Member Plane is untouched.
//!
//! This is the only scenario that boots Kafka (lazily), exercising the full
//! produce → consumer-group → process path end-to-end.

use futures::StreamExt as _;

use crate::chat_it::harness::{
    self, proto, ChatService, HarnessOptions, Request, TestHarness, DEADLINE, KAFKA_DEADLINE,
};

#[tokio::test]
async fn unpublish_event_tears_down_the_audience_plane_clusterwide() {
    let opts = HarnessOptions { with_kafka: true, audience_ttl_secs: 300, ..Default::default() };
    let h = TestHarness::start(opts).await;

    let owner = harness::random_profile();
    let conv = h.create_public_channel(&owner).await;

    let guest = harness::random_profile();
    let mut stream = h.open_public_stream(&conv, &guest).await;

    // Confirm the Audience Plane is fully wired before we tear it down.
    let routing = h.routing.clone();
    harness::await_until("shard activated", DEADLINE, || {
        let routing = routing.clone();
        async move {
            routing
                .active_shards(&conv, harness::now_ms(), 300)
                .await
                .map(|shards| shards.contains(&0u16))
                .unwrap_or(false)
        }
    })
    .await;

    h.send_text(&conv, &owner, "live").await;
    let mut live_seen = false;
    while let Some(item) = harness::recv(&mut stream, DEADLINE).await {
        let resp = item.expect("StreamPublic yielded a non-ok status");
        if resp.message.is_some_and(|m| m.body == "live") {
            live_seen = true;
            break;
        }
    }
    assert!(live_seen, "audience stream never delivered the pre-teardown message");

    // Unpublish → ConversationUnpublished → Kafka → VisibilityWorker.
    ChatService::toggle_visibility(
        &h.handler,
        Request::new(proto::ToggleVisibilityRequest {
            conversation_id: conv.as_str(),
            actor_id:        owner.as_str(),
            make_public:     false,
        }),
    )
    .await
    .expect("toggle_visibility(make_public=false)");

    // The worker clears the routing registry…
    harness::await_until("routing registry cleared by worker", KAFKA_DEADLINE, || {
        let routing = routing.clone();
        async move {
            routing
                .active_shards(&conv, harness::now_ms(), 300)
                .await
                .map(|shards| shards.is_empty())
                .unwrap_or(false)
        }
    })
    .await;

    // …and closes the local audience stream (sender dropped → stream ends).
    let mut torn_down = false;
    loop {
        match tokio::time::timeout(KAFKA_DEADLINE, stream.next()).await {
            Ok(None) => {
                torn_down = true;
                break;
            }
            Ok(Some(_)) => continue, // drain any in-flight frame
            Err(_) => break,         // timed out
        }
    }
    assert!(torn_down, "live guest stream was not cancelled after unpublish");
}
