//! Scenario — handle-claim race (concurrency).
//!
//! A handle is globally unique. When many accounts race to create a profile with
//! the *same* handle, the ScyllaDB LWT in `claim_handle` must let exactly one win;
//! every other create must surface `HandleAlreadyTaken`. This is the concurrency
//! axis: the uniqueness invariant must hold under a genuine simultaneous burst.

use std::sync::Arc;

use crate::profile_it::harness::{self, TestHarness};

const CONTENDERS: usize = 8;

#[tokio::test]
async fn concurrent_creates_of_same_handle_yield_exactly_one_winner() {
    let h = TestHarness::start().await;

    let handle = harness::random_handle();

    // Fire CONTENDERS concurrent creates, distinct accounts, identical handle.
    let mut handles = Vec::new();
    for i in 0..CONTENDERS {
        let bus = Arc::clone(&h.command_bus);
        let handle = handle.clone();
        let account = harness::random_account_id();
        let display = format!("contender-{i}");
        handles.push(tokio::spawn(async move {
            harness::dispatch_create(bus, &account, &handle, &display).await
        }));
    }

    let mut winners = 0;
    let mut losers = 0;
    for handle in handles {
        match handle.await.expect("join") {
            Ok(()) => winners += 1,
            Err(_) => losers += 1,
        }
    }

    assert_eq!(winners, 1, "exactly one create must win the handle claim");
    assert_eq!(losers, CONTENDERS - 1, "every other create must be rejected");

    // The handle resolves to a single, real profile.
    let view = h.get_by_handle(&handle).await;
    assert!(view.is_some(), "the claimed handle must resolve to the winning profile");
}
