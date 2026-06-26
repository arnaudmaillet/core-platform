//! In-memory fakes for the six ports, plus a [`Fixture`] composition root, for the
//! application unit tests. They model the semantics that matter — the ledger's
//! compare-and-append + idempotent lookup, the vault's key lifecycle, the anchor's
//! latest-checkpoint — and expose tampering affordances (corrupt / delete / inject
//! conflicts) so the verify and retry paths are exercised without any container.

use std::collections::{HashSet, VecDeque};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, TimeZone, Utc};

use crate::application::checkpoint::CheckpointHandler;
use crate::application::dto::LedgerQuery;
use crate::application::ingest::IngestHandler;
use crate::application::port::{
    CheckpointAnchor, Clock, EventSource, KeyVault, LedgerStore, WormArchive,
};
use crate::application::privileged::RecordPrivilegedHandler;
use crate::application::query::{ExportHandler, QueryHandler};
use crate::application::shred::CryptoShredHandler;
use crate::application::verify::VerifyHandler;
use crate::domain::event::fixtures;
use crate::domain::{
    AuditEvent, AuditRecord, ChainHead, EventCategory, EventId, MerkleCheckpoint, PartitionKey,
    RecordHash, SubjectKeyRef,
};
use crate::error::AuditError;

// ── Clock ─────────────────────────────────────────────────────────────────────

pub struct FixedClock(DateTime<Utc>);

impl Clock for FixedClock {
    fn now(&self) -> DateTime<Utc> {
        self.0
    }
}

// ── LedgerStore (append-only Postgres analogue) ───────────────────────────────

#[derive(Default)]
pub struct InMemoryLedger {
    records: Mutex<Vec<AuditRecord>>,
    unavailable: AtomicBool,
    /// When set, reads stall indefinitely — models a hung store so the sync lane's
    /// durable-commit deadline (`AUD-4004`) can be exercised.
    hang: AtomicBool,
    /// Number of upcoming appends to reject with ChainHeadConflict (lost-race sim).
    conflicts: AtomicUsize,
}

impl InMemoryLedger {
    pub fn set_unavailable(&self, down: bool) {
        self.unavailable.store(down, Ordering::SeqCst);
    }

    pub fn set_hang(&self, hang: bool) {
        self.hang.store(hang, Ordering::SeqCst);
    }

    pub fn inject_conflicts(&self, n: usize) {
        self.conflicts.store(n, Ordering::SeqCst);
    }

    pub fn record_count(&self) -> usize {
        self.records.lock().unwrap().len()
    }

    /// Mutate a stored record's body in place, keeping its original chain link —
    /// a rogue UPDATE that verification must catch.
    pub fn corrupt_payload_at(&self, partition: &PartitionKey, sequence: u64) {
        let mut tampered = fixtures::draft("tampered", EventCategory::Moderation);
        tampered.action = "rogue.edit".to_owned();
        let tampered = AuditEvent::try_new(tampered).unwrap();
        let mut guard = self.records.lock().unwrap();
        for r in guard.iter_mut() {
            if r.partition() == partition && r.sequence() == sequence {
                *r = r.tampered_clone(tampered.clone());
            }
        }
    }

    /// Remove a record outright — a truncation/splice that leaves a sequence hole.
    pub fn delete_at(&self, partition: &PartitionKey, sequence: u64) {
        self.records
            .lock()
            .unwrap()
            .retain(|r| !(r.partition() == partition && r.sequence() == sequence));
    }

    fn guard(&self) -> Result<(), AuditError> {
        if self.unavailable.load(Ordering::SeqCst) {
            return Err(AuditError::LedgerStoreUnavailable);
        }
        Ok(())
    }

    fn head_of(records: &[AuditRecord], partition: &PartitionKey) -> ChainHead {
        records
            .iter()
            .filter(|r| r.partition() == partition)
            .max_by_key(|r| r.sequence())
            .map(|r| ChainHead::from_parts(r.sequence(), r.record_hash().clone()))
            .unwrap_or_else(ChainHead::genesis)
    }
}

#[async_trait]
impl LedgerStore for InMemoryLedger {
    async fn head(&self, partition: &PartitionKey) -> Result<ChainHead, AuditError> {
        if self.hang.load(Ordering::SeqCst) {
            // Stall far past any test deadline; the timeout cancels this future.
            tokio::time::sleep(std::time::Duration::from_secs(30)).await;
        }
        self.guard()?;
        Ok(Self::head_of(&self.records.lock().unwrap(), partition))
    }

    async fn lookup(&self, event_id: &EventId) -> Result<Option<AuditRecord>, AuditError> {
        self.guard()?;
        Ok(self
            .records
            .lock()
            .unwrap()
            .iter()
            .find(|r| r.event().event_id() == event_id)
            .cloned())
    }

    async fn append(
        &self,
        record: &AuditRecord,
        expected_head: &ChainHead,
    ) -> Result<(), AuditError> {
        self.guard()?;
        // Simulate a lost compare-and-append race.
        if self.conflicts.load(Ordering::SeqCst) > 0 {
            self.conflicts.fetch_sub(1, Ordering::SeqCst);
            return Err(AuditError::ChainHeadConflict {
                partition: record.partition().to_string(),
            });
        }
        let mut guard = self.records.lock().unwrap();
        let current = Self::head_of(&guard, record.partition());
        if current != *expected_head {
            return Err(AuditError::ChainHeadConflict {
                partition: record.partition().to_string(),
            });
        }
        guard.push(record.clone());
        Ok(())
    }

    async fn query(&self, spec: &LedgerQuery) -> Result<Vec<AuditRecord>, AuditError> {
        self.guard()?;
        let guard = self.records.lock().unwrap();
        let mut out: Vec<AuditRecord> = guard
            .iter()
            .filter(|r| {
                let e = r.event();
                spec.subject.as_ref().is_none_or(|s| e.subject() == Some(s))
                    && spec.tenant.as_ref().is_none_or(|t| e.tenant() == Some(t))
                    && spec.category.is_none_or(|c| e.category() == c)
                    && spec.from.is_none_or(|f| e.occurred_at() >= f)
                    && spec.to.is_none_or(|t| e.occurred_at() <= t)
            })
            .cloned()
            .collect();
        if spec.limit > 0 && out.len() > spec.limit {
            out.truncate(spec.limit);
        }
        Ok(out)
    }

    async fn read_partition(
        &self,
        partition: &PartitionKey,
    ) -> Result<Vec<AuditRecord>, AuditError> {
        self.guard()?;
        let guard = self.records.lock().unwrap();
        let mut out: Vec<AuditRecord> = guard
            .iter()
            .filter(|r| r.partition() == partition)
            .cloned()
            .collect();
        out.sort_by_key(|r| r.sequence());
        Ok(out)
    }

    async fn partition_heads(&self) -> Result<Vec<(PartitionKey, RecordHash)>, AuditError> {
        self.guard()?;
        let guard = self.records.lock().unwrap();
        let mut partitions: Vec<PartitionKey> =
            guard.iter().map(|r| r.partition().clone()).collect();
        partitions.sort();
        partitions.dedup();
        Ok(partitions
            .into_iter()
            .map(|p| {
                let hash = Self::head_of(&guard, &p).hash().clone();
                (p, hash)
            })
            .collect())
    }
}

// ── WormArchive (Object-Lock analogue) ────────────────────────────────────────

#[derive(Default)]
pub struct InMemoryArchive {
    archived: Mutex<Vec<AuditRecord>>,
    exports: Mutex<Vec<(String, Vec<u8>)>>,
    unavailable: AtomicBool,
}

impl InMemoryArchive {
    pub fn set_unavailable(&self, down: bool) {
        self.unavailable.store(down, Ordering::SeqCst);
    }

    pub fn archived_count(&self) -> usize {
        self.archived.lock().unwrap().len()
    }

    pub fn export_count(&self) -> usize {
        self.exports.lock().unwrap().len()
    }
}

#[async_trait]
impl WormArchive for InMemoryArchive {
    async fn archive(&self, record: &AuditRecord) -> Result<(), AuditError> {
        if self.unavailable.load(Ordering::SeqCst) {
            return Err(AuditError::ArchiveUnavailable);
        }
        self.archived.lock().unwrap().push(record.clone());
        Ok(())
    }

    async fn store_export(
        &self,
        export_id: &str,
        content: &[u8],
    ) -> Result<String, AuditError> {
        if self.unavailable.load(Ordering::SeqCst) {
            return Err(AuditError::ArchiveUnavailable);
        }
        self.exports
            .lock()
            .unwrap()
            .push((export_id.to_owned(), content.to_vec()));
        Ok(format!("worm://exports/{export_id}"))
    }
}

// ── KeyVault (KMS/HSM analogue) ───────────────────────────────────────────────

#[derive(Default)]
pub struct InMemoryKeyVault {
    keys: Mutex<HashSet<String>>,
    unavailable: AtomicBool,
    fail_destroy: AtomicBool,
}

impl InMemoryKeyVault {
    pub fn seed_key(&self, key_ref: &str) {
        self.keys.lock().unwrap().insert(key_ref.to_owned());
    }

    pub fn exists(&self, key_ref: &str) -> bool {
        self.keys.lock().unwrap().contains(key_ref)
    }

    pub fn set_unavailable(&self, down: bool) {
        self.unavailable.store(down, Ordering::SeqCst);
    }

    pub fn set_fail_destroy(&self, fail: bool) {
        self.fail_destroy.store(fail, Ordering::SeqCst);
    }
}

#[async_trait]
impl KeyVault for InMemoryKeyVault {
    async fn destroy_subject_key(&self, key_ref: &SubjectKeyRef) -> Result<(), AuditError> {
        if self.unavailable.load(Ordering::SeqCst) {
            return Err(AuditError::KeyVaultUnavailable);
        }
        if self.fail_destroy.load(Ordering::SeqCst) {
            return Err(AuditError::CryptoShredFailed {
                subject: key_ref.to_string(),
            });
        }
        self.keys.lock().unwrap().remove(key_ref.as_str());
        Ok(())
    }

    async fn key_exists(&self, key_ref: &SubjectKeyRef) -> Result<bool, AuditError> {
        if self.unavailable.load(Ordering::SeqCst) {
            return Err(AuditError::KeyVaultUnavailable);
        }
        Ok(self.keys.lock().unwrap().contains(key_ref.as_str()))
    }
}

// ── CheckpointAnchor (external witness analogue) ──────────────────────────────

#[derive(Default)]
pub struct InMemoryAnchor {
    latest: Mutex<Option<MerkleCheckpoint>>,
    unavailable: AtomicBool,
}

impl InMemoryAnchor {
    pub fn set_unavailable(&self, down: bool) {
        self.unavailable.store(down, Ordering::SeqCst);
    }
}

#[async_trait]
impl CheckpointAnchor for InMemoryAnchor {
    async fn anchor(&self, checkpoint: &MerkleCheckpoint) -> Result<(), AuditError> {
        if self.unavailable.load(Ordering::SeqCst) {
            return Err(AuditError::AnchorWitnessUnavailable);
        }
        *self.latest.lock().unwrap() = Some(checkpoint.clone());
        Ok(())
    }

    async fn latest_anchored(&self) -> Result<Option<MerkleCheckpoint>, AuditError> {
        if self.unavailable.load(Ordering::SeqCst) {
            return Err(AuditError::AnchorWitnessUnavailable);
        }
        Ok(self.latest.lock().unwrap().clone())
    }
}

// ── EventSource (Kafka feed analogue) ─────────────────────────────────────────

#[derive(Default)]
pub struct FakeEventSource {
    queue: Mutex<VecDeque<AuditEvent>>,
}

impl FakeEventSource {
    pub fn push(&self, event: AuditEvent) {
        self.queue.lock().unwrap().push_back(event);
    }

    pub fn is_drained(&self) -> bool {
        self.queue.lock().unwrap().is_empty()
    }
}

#[async_trait]
impl EventSource for FakeEventSource {
    async fn next_event(&self) -> Result<Option<AuditEvent>, AuditError> {
        Ok(self.queue.lock().unwrap().pop_front())
    }
}

// ── Composition root ──────────────────────────────────────────────────────────

/// Wires the fakes into the application handlers, the way the real composition
/// roots (Phase 5) wire the live adapters.
pub struct Fixture {
    pub ledger: Arc<InMemoryLedger>,
    pub archive: Arc<InMemoryArchive>,
    pub key_vault: Arc<InMemoryKeyVault>,
    pub anchor: Arc<InMemoryAnchor>,
    pub source: Arc<FakeEventSource>,
    pub clock: Arc<FixedClock>,
}

impl Fixture {
    pub fn new() -> Self {
        Self {
            ledger: Arc::new(InMemoryLedger::default()),
            archive: Arc::new(InMemoryArchive::default()),
            key_vault: Arc::new(InMemoryKeyVault::default()),
            anchor: Arc::new(InMemoryAnchor::default()),
            source: Arc::new(FakeEventSource::default()),
            clock: Arc::new(FixedClock(Self::fixed_now())),
        }
    }

    pub fn fixed_now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 6, 26, 12, 0, 0).unwrap()
    }

    pub fn now(&self) -> DateTime<Utc> {
        Self::fixed_now()
    }

    fn ledger_dyn(&self) -> Arc<dyn LedgerStore> {
        self.ledger.clone()
    }

    fn archive_dyn(&self) -> Arc<dyn WormArchive> {
        self.archive.clone()
    }

    fn clock_dyn(&self) -> Arc<dyn Clock> {
        self.clock.clone()
    }

    pub fn ingest(&self) -> IngestHandler {
        IngestHandler::new(self.ledger_dyn(), self.archive_dyn(), self.clock_dyn())
    }

    pub fn privileged(&self) -> RecordPrivilegedHandler {
        RecordPrivilegedHandler::new(self.ledger_dyn(), self.archive_dyn(), self.clock_dyn())
    }

    pub fn shred(&self) -> CryptoShredHandler {
        CryptoShredHandler::new(self.key_vault.clone(), self.clock_dyn())
    }

    pub fn verify(&self) -> VerifyHandler {
        VerifyHandler::new(self.ledger_dyn(), self.anchor.clone())
    }

    pub fn checkpoint(&self) -> CheckpointHandler {
        CheckpointHandler::new(self.ledger_dyn(), self.anchor.clone(), self.clock_dyn())
    }

    pub fn query(&self) -> QueryHandler {
        QueryHandler::new(self.ledger_dyn())
    }

    pub fn export(&self) -> ExportHandler {
        ExportHandler::new(self.ledger_dyn(), self.archive_dyn(), self.clock_dyn())
    }
}

impl Default for Fixture {
    fn default() -> Self {
        Self::new()
    }
}
