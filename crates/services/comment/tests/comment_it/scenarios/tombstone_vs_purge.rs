//! Scenario — tombstone vs purge deletion.
//!
//! Deletion is thread-aware: a leaf comment (no active replies) is *purged* —
//! removed outright — while a comment that still has replies is *tombstoned* —
//! kept with a `Deleted` status so the surrounding thread structure survives.
//! This is the deletion-invariant axis on the flat-tree model.

use crate::comment_it::harness::{self, CommentStatus, TestHarness, DEADLINE};

/// A leaf comment is purged: after deletion it is gone entirely.
#[tokio::test]
async fn deleting_a_leaf_comment_purges_it() {
    let h = TestHarness::start().await;

    let post = harness::random_post();
    let author = harness::random_author();

    let leaf = h.create(&post, None, &author).await;
    assert!(h.get(&leaf).await.is_ok(), "comment exists before deletion");

    h.delete(&leaf, &author).await;

    harness::await_until("leaf comment is purged", DEADLINE, || {
        let h = &h;
        let leaf = &leaf;
        async move { h.get(leaf).await.is_err() }
    })
    .await;
}

/// A comment with active replies is tombstoned: it remains, marked `Deleted`, and
/// its reply survives.
#[tokio::test]
async fn deleting_a_parent_with_replies_tombstones_it() {
    let h = TestHarness::start().await;

    let post = harness::random_post();
    let author = harness::random_author();

    let parent = h.create(&post, None, &author).await;
    let reply = h.create(&post, Some(&parent), &author).await;

    h.delete(&parent, &author).await;

    // The parent is kept as a tombstone (Deleted), not purged.
    harness::await_until("parent is tombstoned, not purged", DEADLINE, || {
        let h = &h;
        let parent = &parent;
        async move {
            matches!(h.get(parent).await, Ok(c) if c.status() == CommentStatus::Deleted)
        }
    })
    .await;

    // The reply is untouched.
    assert!(h.get(&reply).await.is_ok(), "the reply must survive the parent's tombstoning");
}
