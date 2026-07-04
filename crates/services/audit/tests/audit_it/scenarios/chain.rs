//! Live chain integrity over real Postgres + MinIO: the append→chain→archive→verify
//! roundtrip, and detection of a rogue in-place edit.

use audit::application::IntegrityStatus;
use audit::domain::EventCategory;

use crate::audit_it::harness::{Harness, event, fresh_tenant, partition_for};

/// Two events chain over the real Postgres CAS-append (and are archived to real
/// MinIO — a failed PUT would fail the ingest), and the chain verifies.
#[tokio::test]
async fn appends_chain_and_verify_clean() {
    let h = Harness::start().await;
    let tenant = fresh_tenant();

    let p1 = h.ingest().ingest(event(&tenant, "evt-1", EventCategory::Moderation)).await.unwrap();
    let p2 = h.ingest().ingest(event(&tenant, "evt-2", EventCategory::Moderation)).await.unwrap();
    assert_eq!(p1.proof().sequence, 1);
    assert_eq!(p2.proof().sequence, 2);

    let partition = partition_for(&tenant, EventCategory::Moderation);
    let report = h.verify().verify_partition(&partition).await.unwrap();
    assert_eq!(report.status, IntegrityStatus::Verified);
    assert_eq!(report.verified_through, 2);
}

/// A privileged operator editing a row in place (raw UPDATE, bypassing the
/// INSERT-only app grant) is caught by the verifier as a hash mismatch.
#[tokio::test]
async fn in_place_tampering_is_detected() {
    let h = Harness::start().await;
    let tenant = fresh_tenant();

    h.ingest().ingest(event(&tenant, "t-1", EventCategory::Moderation)).await.unwrap();
    h.ingest().ingest(event(&tenant, "t-2", EventCategory::Moderation)).await.unwrap();
    let partition = partition_for(&tenant, EventCategory::Moderation);

    // Clean first.
    assert_eq!(
        h.verify().verify_partition(&partition).await.unwrap().status,
        IntegrityStatus::Verified
    );

    // Rogue UPDATE rewrites record #1's body, leaving its stored hash.
    h.tamper_action(&partition, 1).await;

    let report = h.verify().verify_partition(&partition).await.unwrap();
    assert_eq!(report.status, IntegrityStatus::HashMismatch);
    assert_eq!(report.divergence_at, Some(1));
}
