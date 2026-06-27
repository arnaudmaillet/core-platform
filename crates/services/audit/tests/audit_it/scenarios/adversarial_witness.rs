//! Issue #483 — the adversarial operator simulation, over real infrastructure.
//!
//! Threat model: a database operator who can write *both* the ledger and the
//! Postgres `checkpoint_anchors` pointer (past the INSERT-only app grant). With the
//! v1 Postgres-only anchor such an operator can tamper a row and rewrite the pointer
//! to match — and `verify_global` is fooled. This proves the production
//! [`WitnessCheckpointAnchor`] closes that hole: the **signed** checkpoint anchored
//! to the **independent WORM witness** (a separate MinIO bucket here) is the
//! authority, so the same double-tamper is still caught.
//!
//! Everything here is real: the Postgres ledger + pointer, the MinIO witness, and
//! the signature. (KMS signing is the local HMAC fallback — AC #3 allows running
//! without a real KMS; the witness-authority property is identical.)

use std::sync::Arc;
use std::time::Duration;

use uuid::Uuid;

use audit::application::port::CheckpointAnchor;
use audit::application::{CheckpointHandler, IntegrityStatus, VerifyHandler};
use audit::domain::{EventCategory, MerkleCheckpoint, PartitionKey, RecordHash};
use audit::infrastructure::{
    LocalCheckpointSigner, ObjectLockConfig, ObjectLockWitness, PgCheckpointAnchor, Witness,
    WitnessCheckpointAnchor,
};

use postgres_storage::TransactionManager;

use crate::audit_it::harness::{Harness, event, fresh_tenant, partition_for};

const ANCHOR_INSERT_SQL: &str =
    "INSERT INTO checkpoint_anchors (created_at_ms, checkpoint_json) VALUES ($1, $2)";

/// Build the production witness anchor over a fresh, per-scenario MinIO bucket so the
/// "latest" read is isolated from other parallel scenarios. The HMAC signer is the
/// local fallback (KMS in production); a Postgres convenience pointer rides along —
/// the very pointer the operator will forge.
async fn witness_anchor(h: &Harness, minio: String) -> Arc<dyn CheckpointAnchor> {
    let witness = ObjectLockWitness::new(ObjectLockConfig {
        endpoint: minio,
        region: "us-east-1".into(),
        bucket: format!("witness-{}", Uuid::now_v7()),
        access_key: "minioadmin".into(),
        secret_key: "minioadmin".into(),
        presign_ttl: Duration::from_secs(900),
        request_timeout: Duration::from_secs(10),
    })
    .expect("it: witness");
    witness.ensure_bucket().await.expect("it: witness bucket");

    Arc::new(WitnessCheckpointAnchor::new(
        Arc::new(LocalCheckpointSigner::new([9u8; 32])),
        "local-hmac".to_owned(),
        "HMAC_SHA_256".to_owned(),
        Arc::new(witness) as Arc<dyn Witness>,
        Some(PgCheckpointAnchor::new(TransactionManager::new(h.pool.clone()))),
    ))
}

#[tokio::test]
async fn a_double_tamper_of_ledger_and_postgres_pointer_is_still_caught() {
    let h = Harness::start().await;
    let minio = test_support::containers::minio_ready().await;
    let anchor = witness_anchor(&h, minio).await;

    let tenant = fresh_tenant();
    let partition = partition_for(&tenant, EventCategory::Moderation);
    h.ingest().ingest(event(&tenant, "adv-1", EventCategory::Moderation)).await.unwrap();
    h.ingest().ingest(event(&tenant, "adv-2", EventCategory::Moderation)).await.unwrap();

    // Anchor a signed checkpoint over the honest heads → MinIO witness + PG pointer.
    let checkpoint = CheckpointHandler::new(h.ledger.clone(), anchor.clone(), h.clock.clone());
    let honest = checkpoint.create_and_anchor().await.unwrap();

    // ── The attack, part 1: forge the Postgres pointer with a bogus checkpoint that
    // the operator controls (a later timestamp so it wins the "latest" lookup). ─────
    let forged = MerkleCheckpoint::over(
        &[(
            PartitionKey::new("forged").unwrap(),
            RecordHash::digest(b"operator-controlled"),
        )],
        chrono::Utc::now(),
    );
    sqlx::query(ANCHOR_INSERT_SQL)
        .bind(honest.created_at().timestamp_millis() + 1_000)
        .bind(serde_json::to_string(&forged).unwrap())
        .execute(&h.pool)
        .await
        .expect("it: forge the postgres anchor pointer");

    // A Postgres-only verifier now trusts the forged root (the v1 weakness)...
    let pg_only = PgCheckpointAnchor::new(TransactionManager::new(h.pool.clone()));
    assert_eq!(
        pg_only.latest_anchored().await.unwrap().unwrap().root(),
        forged.root(),
        "the operator successfully forged the Postgres pointer"
    );
    // ...but the witness still returns the honest, signature-verified root.
    let witnessed = anchor.latest_anchored().await.unwrap().expect("a witnessed checkpoint");
    assert_eq!(
        witnessed.root(),
        honest.root(),
        "the external witness is the authority — the Postgres forge had no effect"
    );
    assert_ne!(witnessed.root(), forged.root());

    // ── The attack, part 2: tamper the ledger itself (drop the head — a tail
    // truncation that regresses the partition head below the anchored root). ────────
    h.delete_record(&partition, 2).await;

    // The witness-backed verifier reconciles the live heads against the signed
    // external root the operator could not forge → the double-tamper is caught.
    let verify = VerifyHandler::new(h.ledger.clone(), anchor.clone());
    let report = verify.verify_global().await.unwrap();
    assert_eq!(
        report.status,
        IntegrityStatus::CheckpointDivergence,
        "the signed external witness must catch the operator-level double-tamper"
    );
}
