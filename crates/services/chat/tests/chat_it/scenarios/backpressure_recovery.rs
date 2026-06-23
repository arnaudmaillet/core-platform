//! Scenario 3 — Backpressure & Data-Loss Recovery.
//!
//! A slow consumer that overruns its `broadcast` buffer must get a controlled
//! `data_loss` status (not a panic, not unbounded memory); the pod must stay
//! healthy for everyone else; and the client must recover the full history from
//! the durable ScyllaDB log.
//!
//! Determinism: a parked `broadcast::Receiver` lags only once the sender advances
//! past the buffer, so a tiny buffer + an unpolled stream + `N ≫ buffer` sends is
//! a race-free overflow.

use tonic::Code;

use crate::chat_it::harness::{self, proto, ChatService, HarnessOptions, Request, TestHarness, DEADLINE};

const BUFFER: usize = 4;
const N: usize = 20;

#[tokio::test]
async fn slow_consumer_gets_data_loss_then_recovers_from_scylla() {
    let opts = HarnessOptions { member_buffer: BUFFER, ..Default::default() };
    let h = TestHarness::start(opts).await;

    let owner = harness::random_profile();
    let conv = h.create_public_channel(&owner).await;

    // Open a Member-Plane stream and deliberately never poll it — the starved
    // consumer.
    let mut starved = h.open_member_stream(&conv, &owner).await;

    // Saturate well past the buffer.
    for i in 0..N {
        h.send_text(&conv, &owner, &format!("m{i}")).await;
    }

    // ── Stability: the starved stream surfaces a controlled data_loss ──────────
    let mut saw_data_loss = false;
    // Any buffered `Ok` frames delivered before the lag surfaces are fine.
    while let Some(item) = harness::recv(&mut starved, DEADLINE).await {
        if let Err(status) = item {
            assert_eq!(
                status.code(),
                Code::DataLoss,
                "lagged stream surfaced an unexpected status: {status:?}",
            );
            saw_data_loss = true;
            break;
        }
    }
    assert!(saw_data_loss, "starved stream never surfaced data_loss");

    // ── Recovery: the member repages the full history from ScyllaDB ────────────
    let page = ChatService::get_history(
        &h.handler,
        Request::new(proto::GetHistoryRequest {
            conversation_id: conv.as_str(),
            requester_id:    owner.as_str(),
            limit:           100,
            page_token:      String::new(),
        }),
    )
    .await
    .expect("get_history")
    .into_inner();

    assert_eq!(
        page.messages.len(),
        N,
        "history did not recover every durably-written message",
    );
    // Newest-first, monotonic non-increasing timestamps.
    let mut prev = i64::MAX;
    for view in &page.messages {
        assert!(view.created_at_ms <= prev, "history is not ordered newest-first");
        prev = view.created_at_ms;
    }

    // The hot-tail cache also holds the recent window (a new joiner's fast path).
    let recent = h.hot_tail.recent(&conv, 200).await.expect("hot_tail.recent");
    assert!(recent.len() >= N, "hot-tail cache did not retain the recent window");

    // ── Pod health: a fresh stream still receives live messages ────────────────
    let mut fresh = h.open_member_stream(&conv, &owner).await;
    h.send_text(&conv, &owner, "post-recovery").await;

    let mut healthy = false;
    while let Some(item) = harness::recv(&mut fresh, DEADLINE).await {
        let resp = item.expect("fresh stream yielded a non-ok status");
        if let Some(proto::chat_event::Event::Message(view)) = resp.event.and_then(|e| e.event)
            && view.body == "post-recovery"
        {
            healthy = true;
            break;
        }
    }
    assert!(healthy, "pod did not stay healthy: a fresh stream got no live message after the eviction");
}
