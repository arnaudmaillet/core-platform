//! Outbound ports — the only contracts the application layer holds against the
//! outside world. Concrete adapters (the append-only Postgres ledger, the
//! Object-Lock WORM archive, the KMS/HSM key vault, the external checkpoint
//! anchor, the Kafka event source + decode layer, the system clock) live in
//! `infrastructure` (Phase 4) and are injected as `Arc<dyn …>` at the composition
//! root, so the handlers never name a concrete adapter. Each async port is an
//! `async_trait`; in-memory fakes back the unit tests.
//!
//! * [`LedgerStore`] — the append-only hash-chained System of Record.
//! * [`WormArchive`] — the immutable long-term backstop + export storage.
//! * [`KeyVault`] — per-subject DEK custody; the crypto-shred engine.
//! * [`CheckpointAnchor`] — the bridge to the independent tamper witness.
//! * [`EventSource`] — the async (Kafka) ingestion feed.
//! * [`Clock`] — the injected wall clock.

pub mod checkpoint_anchor;
pub mod clock;
pub mod event_source;
pub mod key_vault;
pub mod ledger_store;
pub mod subject_cipher;
pub mod worm_archive;

pub use checkpoint_anchor::CheckpointAnchor;
pub use clock::Clock;
pub use event_source::EventSource;
pub use key_vault::KeyVault;
pub use ledger_store::LedgerStore;
pub use subject_cipher::SubjectCipher;
pub use worm_archive::WormArchive;
