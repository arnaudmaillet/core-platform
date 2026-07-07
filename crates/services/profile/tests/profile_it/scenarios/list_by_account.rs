//! Scenario — `ListProfilesByAccount` round-trips against ScyllaDB.
//!
//! Regression for the CQL `LIMIT` bind: the limit was cast to `i64`, which the
//! driver serializes as `BigInt` while CQL `LIMIT` is a native `int` — so every
//! call failed type-checking ("expected BigInt") before the query ran. Nothing
//! exercised the repository method, so it slipped through. These tests drive the
//! real bind end-to-end (one profile, and the zero-row case that still binds LIMIT).

use uuid::Uuid;

use profile::domain::value_object::AccountId;

use crate::profile_it::harness::{self, TestHarness};

#[tokio::test]
async fn list_by_account_returns_the_accounts_profile() {
    let h = TestHarness::start().await;

    let account = harness::random_account_id();
    let handle = harness::random_handle();
    h.create(&account, &handle, "Alice").await;

    let account_id = AccountId::from_uuid(Uuid::parse_str(&account).expect("valid account uuid"));
    let (summaries, _next) = h
        .repository
        .list_by_account(&account_id, 50, None)
        .await
        .expect("list_by_account must not error on the LIMIT bind");

    assert_eq!(summaries.len(), 1, "the created profile is listed");
    assert_eq!(summaries[0].handle, handle);
}

#[tokio::test]
async fn list_by_account_is_ok_when_the_account_has_no_profiles() {
    let h = TestHarness::start().await;

    // Binds and executes the same LIMIT-carrying query with zero matching rows —
    // this is what surfaced the type mismatch regardless of result count.
    let account_id = AccountId::from_uuid(Uuid::now_v7());
    let (summaries, next) = h
        .repository
        .list_by_account(&account_id, 50, None)
        .await
        .expect("list_by_account must not error even with zero rows");

    assert!(summaries.is_empty(), "no profiles for a fresh account");
    assert!(next.is_none(), "no next page when empty");
}
