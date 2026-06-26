//! The audit composition roots.
//!
//! [`Adapters::build`] is the I/O variant that constructs the live storage / object
//! -store / vault / anchor adapters from config; both binaries build from it (the
//! server composes the gRPC read/record handler, the worker additionally builds the
//! ingest + checkpoint handlers). [`compose_server`] is *pure* wiring (ports in,
//! gRPC handler out — no I/O), so the same handler is built over fakes or real
//! adapters.

use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use postgres_storage::{PgPoolBuilder, PostgresConfig, TransactionManager};
use sqlx::PgPool;

use crate::application::port::{
    CheckpointAnchor, Clock, KeyVault, LedgerStore, SubjectCipher, WormArchive,
};
use crate::application::{
    CheckpointHandler, CryptoShredHandler, ExportHandler, IngestHandler, QueryHandler,
    RecordPrivilegedHandler, VerifyHandler,
};
use crate::config::AuditConfig;
use crate::infrastructure::grpc::AuditServiceHandler;
use crate::infrastructure::{
    AesGcmSubjectCipher, ObjectLockArchive, PgCheckpointAnchor, PgKeyVault, PgLedger, SystemClock,
};

/// The shared adapter set both binaries build from. Retains the concrete [`PgPool`]
/// so the runtime can build a ledger liveness probe.
pub struct Adapters {
    pub ledger: Arc<dyn LedgerStore>,
    pub archive: Arc<dyn WormArchive>,
    pub key_vault: Arc<dyn KeyVault>,
    pub anchor: Arc<dyn CheckpointAnchor>,
    pub clock: Arc<dyn Clock>,
    /// Seals PII (the moderation rationale) into a crypto-shreddable envelope.
    pub cipher: Arc<dyn SubjectCipher>,
    pub pool: PgPool,
}

impl Adapters {
    /// Connect the canonical ledger (Postgres), the WORM archive (object store), the
    /// key vault and the checkpoint anchor, and wrap them as ports. The Postgres
    /// pool is shared across the three Pg-backed adapters.
    pub async fn build(config: &AuditConfig) -> anyhow::Result<Adapters> {
        let pool = build_pool(config.postgres.clone()).await?;

        let ledger: Arc<dyn LedgerStore> =
            Arc::new(PgLedger::new(TransactionManager::new(pool.clone())));
        let key_vault: Arc<dyn KeyVault> =
            Arc::new(PgKeyVault::new(TransactionManager::new(pool.clone())));
        let anchor: Arc<dyn CheckpointAnchor> =
            Arc::new(PgCheckpointAnchor::new(TransactionManager::new(pool.clone())));
        let archive: Arc<dyn WormArchive> = Arc::new(
            ObjectLockArchive::new(config.object_lock.clone())
                .map_err(|e| anyhow::anyhow!("build WORM archive: {e}"))?,
        );
        let clock: Arc<dyn Clock> = Arc::new(SystemClock);
        let cipher: Arc<dyn SubjectCipher> = Arc::new(AesGcmSubjectCipher::new(
            TransactionManager::new(pool.clone()),
            config.kek,
        ));

        Ok(Adapters {
            ledger,
            archive,
            key_vault,
            anchor,
            clock,
            cipher,
            pool,
        })
    }

    pub fn ingest_handler(&self) -> Arc<IngestHandler> {
        Arc::new(IngestHandler::new(
            Arc::clone(&self.ledger),
            Arc::clone(&self.archive),
            Arc::clone(&self.clock),
        ))
    }

    pub fn checkpoint_handler(&self) -> Arc<CheckpointHandler> {
        Arc::new(CheckpointHandler::new(
            Arc::clone(&self.ledger),
            Arc::clone(&self.anchor),
            Arc::clone(&self.clock),
        ))
    }

    pub fn shred_handler(&self) -> Arc<CryptoShredHandler> {
        Arc::new(CryptoShredHandler::new(
            Arc::clone(&self.key_vault),
            Arc::clone(&self.clock),
        ))
    }
}

/// Pure read/record composition: the four use-case handlers wrapped in the gRPC
/// handler. The `key_vault` is retained for the crypto-shred path (wired when an
/// erasure-request source lands — see the README deferral).
pub fn compose_server(
    ledger: Arc<dyn LedgerStore>,
    archive: Arc<dyn WormArchive>,
    anchor: Arc<dyn CheckpointAnchor>,
    clock: Arc<dyn Clock>,
    record_timeout: Duration,
) -> AuditServiceHandler {
    let record = Arc::new(RecordPrivilegedHandler::new(
        Arc::clone(&ledger),
        Arc::clone(&archive),
        Arc::clone(&clock),
    ));
    let query = Arc::new(QueryHandler::new(Arc::clone(&ledger)));
    let export = Arc::new(ExportHandler::new(
        Arc::clone(&ledger),
        Arc::clone(&archive),
        Arc::clone(&clock),
    ));
    let verify = Arc::new(VerifyHandler::new(Arc::clone(&ledger), Arc::clone(&anchor)));
    AuditServiceHandler::new(record, query, export, verify, record_timeout)
}

async fn build_pool(postgres: PostgresConfig) -> anyhow::Result<PgPool> {
    PgPoolBuilder::build(postgres)
        .await
        .context("build Postgres pool")
}

#[cfg(test)]
mod tests {
    use tonic::{Code, Request};

    use super::*;
    use crate::application::fakes::Fixture;
    use crate::domain::event::fixtures;
    use crate::domain::{AuditEvent, EventCategory};
    use crate::infrastructure::codec;
    use crate::infrastructure::grpc::proto;

    /// Compose the gRPC handler over the in-memory fakes — the same wiring the
    /// binary builds over the live adapters, exercised end-to-end through proto.
    fn handler(fx: &Fixture) -> AuditServiceHandler {
        compose_server(
            fx.ledger.clone(),
            fx.archive.clone(),
            fx.anchor.clone(),
            fx.clock.clone(),
            Duration::from_secs(5),
        )
    }

    fn pb_event(id: &str, category: EventCategory) -> proto::AuditEvent {
        codec::event_to_pb(&AuditEvent::try_new(fixtures::draft(id, category)).unwrap())
    }

    #[tokio::test]
    async fn record_privileged_returns_a_durable_proof() {
        let fx = Fixture::new();
        let req = Request::new(proto::RecordPrivilegedRequest {
            event: Some(pb_event("bg-1", EventCategory::PrivilegedAction)),
            privileged_action: proto::PrivilegedActionType::BreakGlassAccess as i32,
        });
        let resp = handler(&fx).record_privileged(req).await.unwrap().into_inner();
        assert_eq!(resp.sequence_no, 1);
        assert!(!resp.record_hash.is_empty());
        assert_eq!(fx.ledger.record_count(), 1);
    }

    #[tokio::test]
    async fn record_privileged_without_event_is_invalid_argument() {
        let fx = Fixture::new();
        let req = Request::new(proto::RecordPrivilegedRequest {
            event: None,
            privileged_action: proto::PrivilegedActionType::BreakGlassAccess as i32,
        });
        let status = handler(&fx).record_privileged(req).await.unwrap_err();
        assert_eq!(status.code(), Code::InvalidArgument);
    }

    #[tokio::test]
    async fn query_returns_recorded_events_for_a_subject() {
        let fx = Fixture::new();
        // Seed one event through the record path (fixtures::draft uses subject 7f3a).
        handler(&fx)
            .record_privileged(Request::new(proto::RecordPrivilegedRequest {
                event: Some(pb_event("e1", EventCategory::Moderation)),
                privileged_action: proto::PrivilegedActionType::LegalHoldPlace as i32,
            }))
            .await
            .unwrap();

        let resp = handler(&fx)
            .query(Request::new(proto::QueryRequest {
                subject_pseudonym: "7f3a".to_owned(),
                page_size: 50,
                ..Default::default()
            }))
            .await
            .unwrap()
            .into_inner();
        assert_eq!(resp.records.len(), 1);
        assert_eq!(resp.records[0].sequence_no, 1);
    }

    #[tokio::test]
    async fn verify_integrity_global_with_nothing_anchored_is_verified() {
        let fx = Fixture::new();
        let resp = handler(&fx)
            .verify_integrity(Request::new(proto::VerifyIntegrityRequest::default()))
            .await
            .unwrap()
            .into_inner();
        assert_eq!(resp.status, proto::IntegrityStatus::Verified as i32);
    }

    /// Failure simulation (Phase 7): the audit store is wedged when a break-glass
    /// action is attempted. The synchronous lane must FAIL CLOSED — the durable
    /// -commit deadline elapses, the RPC returns DeadlineExceeded (AUD-4004), and
    /// nothing is recorded, so the caller denies the privileged action.
    #[tokio::test]
    async fn break_glass_is_denied_when_audit_cannot_confirm_durability() {
        let fx = Fixture::new();
        fx.ledger.set_hang(true); // the ledger is wedged

        let svc = compose_server(
            fx.ledger.clone(),
            fx.archive.clone(),
            fx.anchor.clone(),
            fx.clock.clone(),
            Duration::from_millis(50), // tight durable-commit deadline
        );

        let status = svc
            .record_privileged(Request::new(proto::RecordPrivilegedRequest {
                event: Some(pb_event("bg-1", EventCategory::PrivilegedAction)),
                privileged_action: proto::PrivilegedActionType::BreakGlassAccess as i32,
            }))
            .await
            .unwrap_err();

        assert_eq!(status.code(), Code::DeadlineExceeded);
        assert_eq!(fx.ledger.record_count(), 0, "nothing may be recorded on a denied break-glass");
    }
}
