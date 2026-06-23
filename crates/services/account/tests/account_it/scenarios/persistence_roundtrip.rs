//! Scenario — durable persistence round-trip (relational write → projection).
//!
//! A create must durably write the account through the `TransactionManager` and
//! be faithfully reflected by the identity projection: email, identity id, and
//! the initial `pending_verification` status. This exercises the full Postgres
//! path — pool, transaction, row mapping, and read projection.

use crate::account_it::harness::{self, TestHarness, DEADLINE};

#[tokio::test]
async fn create_is_persisted_and_reflected_by_the_identity_projection() {
    let h = TestHarness::start().await;

    let identity = harness::random_identity();
    let email = harness::random_email();
    h.create(&identity, &email).await;

    // The identity projection reflects the durable write.
    let identity_q = identity.clone();
    harness::await_until("account readable via the identity projection", DEADLINE, || {
        let h = &h;
        let identity_q = identity_q.clone();
        async move { h.get_by_identity(&identity_q).await.is_ok() }
    })
    .await;

    let view = h.get_by_identity(&identity).await.expect("account exists");
    assert_eq!(view.identity_id, identity, "projection must echo the identity id");
    assert_eq!(view.email, email, "projection must echo the email");
    assert_eq!(
        view.status, "pending_verification",
        "a newly created account starts pending verification",
    );
}
