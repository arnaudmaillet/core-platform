//! The infrastructure tier — the concrete adapters behind the application ports,
//! plus the pure proto/wire mapping layers. This is the only tier that knows about
//! `audit-api`, Postgres, the object store, or Kafka.
//!
//! * [`codec`] — the pure `audit.v1` proto ⇄ domain mapping (gRPC surface). Tested.
//! * [`decode`] — the audit-owned JSON wire schema for `audit.v1.events` ⇄ domain
//!   (the async ingest lane). Tested.
//! * [`pg_ledger`] — the append-only, hash-chained Postgres [`LedgerStore`] with
//!   compare-and-append; `UPDATE`/`DELETE` revoked at the role level.
//! * [`object_lock_archive`] — the S3/MinIO Object-Lock (compliance-mode)
//!   [`WormArchive`].
//! * [`pg_key_vault`] — the per-subject DEK [`KeyVault`] (v1 over Postgres;
//!   production = KMS/HSM in a separate trust domain).
//! * [`pg_anchor`] — the [`CheckpointAnchor`] (v1 over Postgres; production =
//!   RFC 3161 / cross-account WORM witness).
//! * [`consumer`] — the `run_consumer` ingest-lane wiring + the `ClassifyError`
//!   impl that drives retry/DLQ.
//!
//! [`LedgerStore`]: crate::application::port::LedgerStore
//! [`WormArchive`]: crate::application::port::WormArchive
//! [`KeyVault`]: crate::application::port::KeyVault
//! [`CheckpointAnchor`]: crate::application::port::CheckpointAnchor

pub mod codec;
pub mod consumer;
pub mod decode;
pub mod object_lock_archive;
pub mod pg_anchor;
pub mod pg_key_vault;
pub mod pg_ledger;

pub use consumer::run_audit_ingest_consumer;
pub use decode::{AuditEventWire, map_audit_event};
pub use object_lock_archive::{ObjectLockArchive, ObjectLockConfig};
pub use pg_anchor::PgCheckpointAnchor;
pub use pg_key_vault::PgKeyVault;
pub use pg_ledger::PgLedger;
