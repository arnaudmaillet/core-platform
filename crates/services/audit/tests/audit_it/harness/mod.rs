//! Integration harness: boots ephemeral Postgres + MinIO (Object Lock), applies
//! the `.sql` migrations, and wires the real audit adapters and handlers against
//! them. Isolation is by a fresh per-scenario tenant (UUID) — each scenario's
//! chain partition is unique, so the shared containers run every scenario in
//! parallel.
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, TimeZone, Utc};
use uuid::Uuid;

use audit::application::port::{
    CheckpointAnchor, Clock, KeyVault, LedgerStore, SubjectCipher, WormArchive,
};
use audit::application::{
    CheckpointHandler, CryptoShredHandler, IngestHandler, QueryHandler, VerifyHandler,
};
use audit::domain::{
    Actor, ActorType, AuditEvent, EventCategory, EventId, LawfulBasis, NewAuditEvent, Outcome,
    PartitionKey, PiiEnvelope, ResourceRef, SubjectKeyRef, SubjectPseudonym, TenantId,
};
use audit::infrastructure::{
    AesGcmSubjectCipher, ObjectLockArchive, ObjectLockConfig, PgCheckpointAnchor, PgKeyVault,
    PgLedger, SystemClock,
};

use postgres_storage::config::StatementLogLevel;
use postgres_storage::{PgPoolBuilder, PostgresConfig, TransactionManager};
use sqlx::PgPool;

const PG_MIGRATIONS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/migrations");

/// The action verb every harness-built event carries — a known sentinel so the
/// tamper scenario can rewrite it via raw SQL.
pub const LIVE_ACTION: &str = "live.action";

/// A fixed test KEK (production reads a real one from env / KMS).
const TEST_KEK: [u8; 32] = [42u8; 32];

pub struct Harness {
    pub ledger: Arc<dyn LedgerStore>,
    pub archive: Arc<dyn WormArchive>,
    pub key_vault: Arc<dyn KeyVault>,
    pub anchor: Arc<dyn CheckpointAnchor>,
    pub clock: Arc<dyn Clock>,
    pub cipher: Arc<dyn SubjectCipher>,
    pub pool: PgPool,
}

impl Harness {
    pub async fn start() -> Self {
        let pg_url = test_support::containers::postgres_ready(PG_MIGRATIONS).await;
        let minio_endpoint = test_support::containers::minio_ready().await;

        let pg_config = PostgresConfig {
            database_url: pg_url,
            max_connections: 8,
            min_connections: 1,
            acquire_timeout: Duration::from_secs(5),
            idle_timeout: None,
            max_lifetime: None,
            statement_log_level: StatementLogLevel::Debug,
            slow_statement_threshold: Duration::from_millis(500),
        };
        let pool = PgPoolBuilder::build(pg_config).await.expect("it: pg pool");

        let archive = ObjectLockArchive::new(ObjectLockConfig {
            endpoint: minio_endpoint,
            region: "us-east-1".into(),
            bucket: "audit-archive".into(),
            access_key: "minioadmin".into(),
            secret_key: "minioadmin".into(),
            presign_ttl: Duration::from_secs(900),
            request_timeout: Duration::from_secs(10),
        })
        .expect("it: object store");
        archive.ensure_bucket().await.expect("it: create bucket");

        let ledger: Arc<dyn LedgerStore> =
            Arc::new(PgLedger::new(TransactionManager::new(pool.clone())));
        let key_vault: Arc<dyn KeyVault> =
            Arc::new(PgKeyVault::new(TransactionManager::new(pool.clone())));
        let anchor: Arc<dyn CheckpointAnchor> =
            Arc::new(PgCheckpointAnchor::new(TransactionManager::new(pool.clone())));
        let archive: Arc<dyn WormArchive> = Arc::new(archive);
        let clock: Arc<dyn Clock> = Arc::new(SystemClock);
        let cipher: Arc<dyn SubjectCipher> = Arc::new(AesGcmSubjectCipher::new(
            TransactionManager::new(pool.clone()),
            TEST_KEK,
        ));

        Self {
            ledger,
            archive,
            key_vault,
            anchor,
            clock,
            cipher,
            pool,
        }
    }

    pub fn ingest(&self) -> IngestHandler {
        IngestHandler::new(self.ledger.clone(), self.archive.clone(), self.clock.clone())
    }

    pub fn verify(&self) -> VerifyHandler {
        VerifyHandler::new(self.ledger.clone(), self.anchor.clone())
    }

    pub fn shred(&self) -> CryptoShredHandler {
        CryptoShredHandler::new(self.key_vault.clone(), self.clock.clone())
    }

    pub fn checkpoint(&self) -> CheckpointHandler {
        CheckpointHandler::new(self.ledger.clone(), self.anchor.clone(), self.clock.clone())
    }

    pub fn query(&self) -> QueryHandler {
        QueryHandler::new(self.ledger.clone())
    }

    /// Seed a per-subject DEK so a later shred has something to destroy.
    pub async fn seed_key(&self, key_ref: &str) {
        sqlx::query("INSERT INTO subject_keys (key_ref, created_at_ms) VALUES ($1, $2)")
            .bind(key_ref)
            .bind(Utc::now().timestamp_millis())
            .execute(&self.pool)
            .await
            .expect("it: seed key");
    }

    /// Simulate a privileged operator editing a row in place (the INSERT-only grant
    /// stops the app; this is the DBA-with-credentials threat). Rewrites the event
    /// body inside record_json while leaving the stored chain link — so the
    /// recomputed hash no longer matches.
    pub async fn tamper_action(&self, partition: &PartitionKey, sequence: i64) {
        let n = sqlx::query(
            "UPDATE audit_records \
             SET record_json = replace(record_json, $1, 'live.TAMPERED') \
             WHERE partition_key = $2 AND sequence_no = $3",
        )
        .bind(LIVE_ACTION)
        .bind(partition.as_str())
        .bind(sequence)
        .execute(&self.pool)
        .await
        .expect("it: tamper");
        assert_eq!(n.rows_affected(), 1, "tamper should hit exactly one row");
    }

    /// Delete a record outright — a tail truncation the head checkpoint catches.
    pub async fn delete_record(&self, partition: &PartitionKey, sequence: i64) {
        sqlx::query("DELETE FROM audit_records WHERE partition_key = $1 AND sequence_no = $2")
            .bind(partition.as_str())
            .bind(sequence)
            .execute(&self.pool)
            .await
            .expect("it: delete");
    }
}

// ── Builders ──────────────────────────────────────────────────────────────────

/// A fresh, scenario-isolated tenant (so its derived chain partition is unique).
pub fn fresh_tenant() -> TenantId {
    TenantId::new(format!("t-{}", Uuid::now_v7())).unwrap()
}

pub fn partition_for(tenant: &TenantId, category: EventCategory) -> PartitionKey {
    PartitionKey::derive(Some(tenant), category)
}

pub fn at(ms: i64) -> DateTime<Utc> {
    Utc.timestamp_millis_opt(ms).single().unwrap()
}

/// Build a minimal valid event in `tenant`'s scope.
pub fn event(tenant: &TenantId, id: &str, category: EventCategory) -> AuditEvent {
    AuditEvent::try_new(NewAuditEvent {
        event_id: EventId::new(id).unwrap(),
        category,
        subject: Some(SubjectPseudonym::new("subj-1").unwrap()),
        tenant: Some(tenant.clone()),
        actor: Actor::new(ActorType::Admin, audit::domain::ActorPseudonym::new("adm-1").unwrap(), "s-1"),
        action: LIVE_ACTION.to_owned(),
        resource: ResourceRef::new("account", "acc-1"),
        outcome: Outcome::Executed,
        lawful_basis: LawfulBasis::LegalObligation,
        source_service: "moderation".to_owned(),
        correlation_id: "trace-1".to_owned(),
        occurred_at: at(1_750_000_000_000),
        pii: None,
        attributes: BTreeMap::new(),
    })
    .unwrap()
}

/// An event carrying a sealed PII envelope keyed by `key_ref`.
pub fn pii_event(tenant: &TenantId, id: &str, key_ref: &str) -> AuditEvent {
    AuditEvent::try_new(NewAuditEvent {
        event_id: EventId::new(id).unwrap(),
        category: EventCategory::DataAccess,
        subject: Some(SubjectPseudonym::new("subj-1").unwrap()),
        tenant: Some(tenant.clone()),
        actor: Actor::new(ActorType::Admin, audit::domain::ActorPseudonym::new("adm-1").unwrap(), "s-1"),
        action: LIVE_ACTION.to_owned(),
        resource: ResourceRef::new("account", "acc-1"),
        outcome: Outcome::Permitted,
        lawful_basis: LawfulBasis::LegalObligation,
        source_service: "account".to_owned(),
        correlation_id: "trace-1".to_owned(),
        occurred_at: at(1_750_000_000_000),
        pii: Some(PiiEnvelope::sealed(
            SubjectKeyRef::new(key_ref).unwrap(),
            b"ciphertext".to_vec(),
            b"nonce".to_vec(),
            "AES-256-GCM",
        )),
        attributes: BTreeMap::new(),
    })
    .unwrap()
}
