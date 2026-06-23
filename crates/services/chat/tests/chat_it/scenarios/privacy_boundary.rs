//! Scenario 1 — The Privacy Boundary (Shadowing validation).
//!
//! Asserts that an Audience-Plane reader (`StreamPublic`) receives message
//! shadows but is *structurally* shielded from Member-Plane noise (typing,
//! presence, receipts), while a Member-Plane reader receives all of it.
//!
//! The strong assertion is at the **transport** layer, not the client filter: we
//! tap the in-process Audience registry directly and inspect every `PlaneEvent`
//! variant that reaches it. Member-only signals are published exclusively to the
//! member channel, so they can never appear on the audience side — and if a
//! regression made `dispatch_member_signal` also hit the audience channel, the
//! leaked signal (sent *before* the sentinel message) would arrive first and the
//! tap would catch it.

use crate::chat_it::harness::{
    self, proto, ChatService, HarnessOptions, PlaneEvent, Request, TestHarness, DEADLINE,
};

#[tokio::test]
async fn audience_gets_message_shadow_but_never_member_plane_signals() {
    let h = TestHarness::start(HarnessOptions::default()).await;

    let owner = harness::random_profile();
    let conv = h.create_public_channel(&owner).await;

    // Owner attaches to the Member Plane; guest attaches to the Audience Plane.
    // Held (not polled) so the member channel stays subscribed for the duration.
    let member_stream = h.open_member_stream(&conv, &owner).await;
    let guest = harness::random_profile();
    let mut public_stream = h.open_public_stream(&conv, &guest).await;

    // Raw taps onto the in-process registries to inspect event *variants*.
    let mut member_tap = h.member_registry.subscribe(&conv);
    let mut audience_tap = h.audience_registry.subscribe(&conv);

    // Member-Plane-only signals first, then a sentinel message last.
    ChatService::send_typing(
        &h.handler,
        Request::new(proto::SendTypingRequest {
            conversation_id: conv.as_str(),
            member_id:       owner.as_str(),
        }),
    )
    .await
    .expect("send_typing");
    ChatService::heartbeat(
        &h.handler,
        Request::new(proto::HeartbeatRequest {
            conversation_id: conv.as_str(),
            member_id:       owner.as_str(),
        }),
    )
    .await
    .expect("heartbeat");
    ChatService::mark_read(
        &h.handler,
        Request::new(proto::MarkReadRequest {
            conversation_id: conv.as_str(),
            member_id:       owner.as_str(),
            message_id:      harness::random_message_id(),
        }),
    )
    .await
    .expect("mark_read");

    h.send_text(&conv, &owner, "sentinel").await;

    // ── Audience tap: every event must be a Message; the sentinel must arrive ──
    let mut saw_sentinel = false;
    while let Some(event) = harness::recv_event(&mut audience_tap, DEADLINE).await {
        match event.as_ref() {
            PlaneEvent::Message(frame) if frame.body == "sentinel" => {
                saw_sentinel = true;
                break;
            }
            PlaneEvent::Message(_) => {}
            leaked => panic!("Audience Plane leaked a Member-Plane event: {leaked:?}"),
        }
    }
    assert!(saw_sentinel, "audience tap never observed the shadow message");

    // ── The actual StreamPublic stream delivers the shadow message ─────────────
    let mut delivered = false;
    while let Some(item) = harness::recv(&mut public_stream, DEADLINE).await {
        let resp = item.expect("StreamPublic yielded a non-ok status");
        if resp.message.is_some_and(|m| m.body == "sentinel") {
            delivered = true;
            break;
        }
    }
    assert!(delivered, "StreamPublic never delivered the shadow message");

    // ── Member tap: proves the signals were actually emitted (asymmetry is real)
    let (mut msg, mut typing, mut presence, mut receipt) = (false, false, false, false);
    while !(msg && typing && presence && receipt) {
        match harness::recv_event(&mut member_tap, DEADLINE).await {
            Some(event) => match event.as_ref() {
                PlaneEvent::Message(_) => msg = true,
                PlaneEvent::Typing { .. } => typing = true,
                PlaneEvent::Presence { .. } => presence = true,
                PlaneEvent::Receipt { .. } => receipt = true,
            },
            None => break,
        }
    }
    assert!(
        msg && typing && presence && receipt,
        "Member Plane missing variants — msg={msg} typing={typing} presence={presence} receipt={receipt}",
    );

    // Keep the member stream alive until the end so its channel stays subscribed.
    drop(member_stream);
}
