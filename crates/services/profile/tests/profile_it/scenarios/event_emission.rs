//! Scenario — outbound event emission.
//!
//! Every mutating command must publish its lifecycle event to `profile.v1.events`
//! *after* the durable write, so downstream consumers (notably search) can react.
//! The harness injects a capturing publisher that records each event's `type` tag.

use crate::profile_it::harness::{self, TestHarness};

#[tokio::test]
async fn create_then_update_emit_their_lifecycle_events() {
    let h = TestHarness::start().await;
    let handle = harness::random_handle();
    let account = harness::random_account_id();

    h.create(&account, &handle, "Alice").await;
    assert!(
        h.publisher.published().contains(&"ProfileCreated".to_owned()),
        "create must emit ProfileCreated, got {:?}",
        h.publisher.published()
    );

    let view = h.get_by_handle(&handle).await.expect("profile exists after create");
    h.update_display(&view.id, "Alice Updated").await;
    assert!(
        h.publisher.published().contains(&"ProfileUpdated".to_owned()),
        "update must emit ProfileUpdated, got {:?}",
        h.publisher.published()
    );
}
