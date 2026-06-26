//! Live idempotency over real Postgres: a redelivered event (same deterministic
//! id) is deduped to the original proof and chained exactly once — proving the
//! `event_id` unique constraint + the lookup dedupe behave as the domain assumes.

use audit::domain::EventCategory;

use crate::audit_it::harness::{Harness, event, fresh_tenant};

#[tokio::test]
async fn redelivery_is_deduped() {
    let h = Harness::start().await;
    let tenant = fresh_tenant();

    let first = h.ingest().ingest(event(&tenant, "dup-1", EventCategory::Authentication)).await.unwrap();
    let again = h.ingest().ingest(event(&tenant, "dup-1", EventCategory::Authentication)).await.unwrap();

    assert!(!first.is_duplicate());
    assert!(again.is_duplicate());
    assert_eq!(first.proof(), again.proof());
    assert_eq!(first.proof().sequence, 1);
}
