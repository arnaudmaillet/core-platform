//! ScyllaDB evidence-history adapter — the append-only audit feed projected from
//! the moderation event stream.

pub mod scylla_evidence_history;

pub use scylla_evidence_history::ScyllaEvidenceHistory;
