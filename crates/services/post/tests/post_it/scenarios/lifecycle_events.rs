//! Scenario — post lifecycle transitions and domain-event emission.
//!
//! A post is born `Draft`; publishing it transitions to `Published` and emits a
//! `PostPublished` event; deleting it transitions to `Deleted` and emits a
//! `PostDeleted` event. The capturing publisher records each emission, so this
//! asserts both the persisted status transitions and the outbound event contract
//! that downstream services (timeline, notification) depend on.

use crate::post_it::harness::{self, PostStatus, TestHarness};

#[tokio::test]
async fn create_publish_delete_transitions_status_and_emits_events() {
    let h = TestHarness::start().await;

    let profile_id = harness::random_id();
    let post_id = harness::random_id();

    // Create → Draft, no events yet.
    h.create(&post_id, &profile_id).await;
    let post = h.get(&post_id).await.expect("post exists after create");
    assert_eq!(post.status(), PostStatus::Draft, "a freshly created post is a draft");
    assert!(h.publisher.labels().is_empty(), "create must not emit a domain event");

    // Publish → Published, one PostPublished event.
    h.publish(&post_id, &profile_id).await;
    let post = h.get(&post_id).await.expect("post exists after publish");
    assert_eq!(post.status(), PostStatus::Published, "publish must transition to Published");
    assert_eq!(h.publisher.count("published"), 1, "publish must emit exactly one PostPublished");

    // Delete → Deleted, one PostDeleted event.
    h.delete(&post_id, &profile_id).await;
    let post = h.get(&post_id).await.expect("tombstone row remains readable after delete");
    assert_eq!(post.status(), PostStatus::Deleted, "delete must transition to Deleted");
    assert_eq!(h.publisher.count("deleted"), 1, "delete must emit exactly one PostDeleted");
}
