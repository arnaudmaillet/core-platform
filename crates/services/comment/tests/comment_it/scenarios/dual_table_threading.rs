//! Scenario — dual-table threading consistency.
//!
//! A comment is written to the canonical `comments` table and the
//! `comments_by_post` thread index. A top-level comment must appear in the post's
//! top-level listing and be readable by id; a reply must appear under its parent's
//! reply listing. The two projections of the thread must agree. This is the
//! dual-table / temporal-ordering axis.

use crate::comment_it::harness::{self, TestHarness, DEADLINE};

#[tokio::test]
async fn top_level_and_reply_are_consistent_across_tables() {
    let h = TestHarness::start().await;

    let post = harness::random_post();
    let author = harness::random_author();

    // A top-level comment is readable by id and listed under the post.
    let top = h.create(&post, None, &author).await;
    assert!(h.get(&top).await.is_ok(), "top-level comment must be readable by id");

    harness::await_until("top-level comment appears in the post listing", DEADLINE, || {
        let h = &h;
        let post = &post;
        let top = &top;
        async move { harness::summaries_contain(&h.list_top_level(post).await, top) }
    })
    .await;

    // A reply is listed under its parent — not in the top-level listing.
    let reply = h.create(&post, Some(&top), &author).await;

    harness::await_until("reply appears under its parent", DEADLINE, || {
        let h = &h;
        let post = &post;
        let top = &top;
        let reply = &reply;
        async move { harness::summaries_contain(&h.list_replies(post, top).await, reply) }
    })
    .await;

    assert!(
        !harness::summaries_contain(&h.list_top_level(&post).await, &reply),
        "a reply must not appear in the post's top-level listing",
    );
}
