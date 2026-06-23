//! Scenario — email uniqueness race (concurrency / transactional integrity).
//!
//! The create handler guards uniqueness with a check-then-insert, but the
//! authoritative guard is the Postgres unique index on `email`: when many
//! sign-ups race for the same address, exactly one INSERT may commit and the rest
//! must surface a conflict. This is the concurrency axis on the relational
//! backend — the DB constraint, not the application check, is what holds.

use std::sync::Arc;

use crate::account_it::harness::{self, TestHarness};

const CONTENDERS: usize = 6;

#[tokio::test]
async fn concurrent_creates_of_same_email_yield_exactly_one_winner() {
    let h = TestHarness::start().await;

    let email = harness::random_email();
    let identities: Vec<String> = (0..CONTENDERS).map(|_| harness::random_identity()).collect();

    // Fire CONTENDERS concurrent creates: distinct identities, identical email.
    let mut handles = Vec::new();
    for identity in &identities {
        let bus = Arc::clone(&h.command_bus);
        let identity = identity.clone();
        let email = email.clone();
        handles.push(tokio::spawn(harness::dispatch_create(bus, identity, email)));
    }

    let mut winners = 0;
    for handle in handles {
        if handle.await.expect("join").is_ok() {
            winners += 1;
        }
    }
    assert_eq!(winners, 1, "the Postgres unique index must admit exactly one create for the email");

    // Persistence cross-check: exactly one identity resolved to a stored account.
    let mut persisted = 0;
    for identity in &identities {
        if h.get_by_identity(identity).await.is_ok() {
            persisted += 1;
        }
    }
    assert_eq!(persisted, 1, "exactly one account must be durably persisted for the contested email");
}
