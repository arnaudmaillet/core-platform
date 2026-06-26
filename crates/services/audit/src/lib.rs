//! `audit` — the platform's **tamper-evident compliance evidence plane**: the
//! append-only, hash-chained System of Record that answers *"who did what, to
//! whom, when, under what authority, and with what outcome"* for every security-,
//! privacy- and regulatory-relevant event in the fleet.
//!
//! This service is emphatically **not** "logging, but serious". Application
//! telemetry (traces, metrics, debug logs) and a compliance trail are two
//! different substances that merely look alike on a screen, and conflating them
//! is a category error a hyperscale system punishes:
//!
//! * **telemetry** is best-effort, mutable, sampled, retention-cycled, and read
//!   by every engineer — correct for observability, fatal for evidence.
//! * an **audit trail** must be zero-loss for in-scope events, append-only,
//!   tamper-evident, provably complete, retained for years, access-controlled
//!   (and its own reads audited), and erasable at the *field* level without
//!   destroying the record — so a GDPR Art. 17 request and the Art. 5(2)
//!   accountability duty stop being a contradiction.
//!
//! That framing sets its boundaries:
//!
//! * it records **decisions and privileged actions** emitted by the services that
//!   understand them (`moderation` enforcement, `auth` issuance / break-glass,
//!   `account` consent + PII lifecycle, and privileged actions fleet-wide) — it
//!   is a *recorder*, never an *actor*: it makes no business decision.
//! * it **owns** the immutable ledger, the integrity proofs (per-partition hash
//!   chains + periodic Merkle checkpoints signed in a separate KMS trust domain +
//!   external anchoring), the retention / legal-hold policy, the crypto-shred
//!   lifecycle, and the access-controlled query/export surface for DPO / internal
//!   audit / regulators.
//! * it **never** stores raw business content (post bodies, media bytes, message
//!   text — only references + decision metadata), cleartext PII inside the chain
//!   (only pseudonyms + per-subject crypto-shreddable envelopes), or application
//!   telemetry. It is **not** a log aggregator and **not** an analytics store.
//!
//! ## Posture — TIER-0, deliberately split
//! Audit is a **TIER-0** legal/regulatory-critical plane with a *split* posture:
//! **fail-open at producers** (the async Kafka lane — audit liveness can never
//! brown out the business mesh; Kafka is the durable buffer that absorbs write
//! spikes) but **fail-closed on durability and on the narrow synchronous
//! break-glass lane** (the most dangerous actions — break-glass access, legal-hold
//! placement, consent changes — are *denied* if they cannot be provably recorded
//! first). You should not be able to do the most dangerous things in the system
//! precisely when the system can't prove you did them.
//!
//! The architectural commitment is **two deployables** that share this domain
//! crate but no process or failure domain: a **read/record server**
//! (`audit-server`, the gRPC query/export reads + the synchronous fail-closed
//! `RecordPrivileged` RPC on :50068) and an **ingest/verify worker**
//! (`audit-worker`, the `run_consumer` ingestion lane that chains + persists +
//! archives, plus the supervised verifier / checkpoint-anchor / retention /
//! crypto-shred loops). See `project_audit_service_blueprint` for the full design.
//!
//! ## Module roadmap (built phase by phase)
//! Phase 0 (now): [`error`] — the canonical `AUD-XXXX` namespace — and [`service`]
//! — the two health-only [`service_runtime::Service`] stubs.
//! Phase 1: the `audit.v1` event/query contract (`audit-api`) — the sync
//! `RecordPrivileged` RPC, the `Query`/`Export`/`VerifyIntegrity` reads, and the
//! `audit.v1.events` Kafka envelope. · Phase 2: `domain` (AuditRecord, the
//! `ChainLink`/`HashChain` tamper-evidence VOs, `MerkleCheckpoint`, the
//! `SubjectKeyRef` crypto-shred VO, `RetentionPolicy` / `LegalHold`, and the
//! per-partition sequence/gap invariants — pure). · Phase 3: `application` + ports
//! (LedgerStore / WormArchive / KeyVault+Shredder / CheckpointAnchor / EventSource,
//! the ingest, record-privileged [sync, fail-closed], crypto-shred, verify-chain
//! and query/export handlers, plus in-memory fakes). · Phase 4: `infrastructure` (the
//! append-only Postgres ledger with revoked UPDATE/DELETE grants, the S3/MinIO
//! Object-Lock WORM archive, the KMS/HSM signer + per-subject DEK vault, the
//! external anchor/witness, the `run_consumer` ingestion lane). · Phase 5: `app`
//! (composition roots) + the two runtime wirings in [`service`].

pub mod application;
pub mod domain;
pub mod error;
pub mod service;

pub use error::AuditError;
pub use service::{AuditServerService, AuditWorkerService};
