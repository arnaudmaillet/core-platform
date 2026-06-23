//! Scenario — dual-table write consistency under concurrency.
//!
//! A create writes the canonical `posts` row *and* the `posts_by_profile` index
//! row. The two must never diverge: every post readable by id must also appear in
//! its author's listing, and vice versa — even when many creates for one profile
//! race. This is the concurrency / dual-table axis.

use std::collections::HashSet;
use std::sync::Arc;

use crate::post_it::harness::{self, TestHarness, DEADLINE};

const BURST: usize = 6;

#[tokio::test]
async fn concurrent_creates_keep_both_tables_in_agreement() {
    let h = TestHarness::start().await;

    let profile_id = harness::random_id();
    let post_ids: Vec<String> = (0..BURST).map(|_| harness::random_id()).collect();

    // Fire BURST concurrent creates for the same profile.
    let mut handles = Vec::new();
    for post_id in &post_ids {
        let bus = Arc::clone(&h.command_bus);
        let post_id = post_id.clone();
        let profile_id = profile_id.clone();
        handles.push(tokio::spawn(harness::dispatch_create(bus, post_id, profile_id)));
    }
    for handle in handles {
        handle.await.expect("join").expect("create_post");
    }

    // The `posts_by_profile` index must list exactly the created set.
    let expected: HashSet<&str> = post_ids.iter().map(String::as_str).collect();
    harness::await_until("posts_by_profile lists every created post", DEADLINE, || {
        let h = &h;
        let profile_id = profile_id.clone();
        let expected = expected.clone();
        async move {
            let listed: HashSet<String> =
                h.list(&profile_id).await.into_iter().map(|s| s.post_id.as_str()).collect();
            listed.len() == expected.len()
                && expected.iter().all(|id| listed.contains(*id))
        }
    })
    .await;

    // Every listed post is also readable by id from the canonical `posts` table.
    for post_id in &post_ids {
        let post = h.get(post_id).await.expect("post readable by id from `posts`");
        assert_eq!(post.id().as_str(), *post_id);
    }
}
