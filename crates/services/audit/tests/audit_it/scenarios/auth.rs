//! Live auth→audit path over real Postgres + MinIO: the authentication lifecycle
//! (`session_issued` / `session_revoked`) maps into the chain and verifies. Auth
//! events carry no free-text PII, so there is no sealing here.

use audit::application::IntegrityStatus;
use audit::domain::{EventCategory, PartitionKey};
use audit::infrastructure::auth_decode::{SessionIssuedWire, SessionRevokedWire};
use audit::infrastructure::{map_session_issued, map_session_revoked};
use uuid::Uuid;

use crate::audit_it::harness::{Harness, at};

#[tokio::test]
async fn auth_session_lifecycle_chains_and_verifies() {
    let h = Harness::start().await;
    let account = Uuid::now_v7().to_string();
    let session = Uuid::now_v7().to_string();

    let issued = SessionIssuedWire {
        session_id: session.clone(),
        account_id: account.clone(),
        generation: 1,
        expires_at: at(1_750_003_600_000),
        absolute_expiry: at(1_750_086_400_000),
        occurred_at: at(1_750_000_000_000),
        correlation_id: Uuid::now_v7().to_string(),
    };
    let issued_event = map_session_issued(&issued).unwrap();
    assert!(!issued_event.has_pii(), "auth events carry no PII");
    h.ingest().ingest(issued_event).await.unwrap();

    let revoked = SessionRevokedWire {
        session_id: session.clone(),
        account_id: account.clone(),
        generation: 1,
        reason: "logout".to_owned(),
        occurred_at: at(1_750_000_001_000),
        correlation_id: Uuid::now_v7().to_string(),
    };
    h.ingest().ingest(map_session_revoked(&revoked).unwrap()).await.unwrap();

    // Auth records share the tenant-less Authentication partition; it verifies.
    let partition = PartitionKey::derive(None, EventCategory::Authentication);
    assert_eq!(
        h.verify().verify_partition(&partition).await.unwrap().status,
        IntegrityStatus::Verified
    );
}

#[tokio::test]
async fn auth_session_issued_replay_is_deduped() {
    let h = Harness::start().await;
    let session = Uuid::now_v7().to_string();
    let issued = SessionIssuedWire {
        session_id: session.clone(),
        account_id: Uuid::now_v7().to_string(),
        generation: 1,
        expires_at: at(1_750_003_600_000),
        absolute_expiry: at(1_750_086_400_000),
        occurred_at: at(1_750_000_000_000),
        correlation_id: Uuid::now_v7().to_string(),
    };

    let first = h.ingest().ingest(map_session_issued(&issued).unwrap()).await.unwrap();
    let again = h.ingest().ingest(map_session_issued(&issued).unwrap()).await.unwrap();

    assert!(!first.is_duplicate());
    assert!(again.is_duplicate());
    assert_eq!(first.proof(), again.proof());
}
