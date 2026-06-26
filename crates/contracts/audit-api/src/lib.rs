//! `audit-api` — the generated contract for `audit.v1` (server + client stubs +
//! descriptor), compiled from the shared `contracts/proto` IDL. Consumers depend
//! on this crate instead of recompiling the `.proto` files.
//!
//! The audit surface is deliberately narrow and asymmetric — most audit traffic
//! never reaches gRPC at all:
//!
//! * **One canonical envelope, two lanes.** [`AuditEvent`] is what every producer
//!   emits. On the **async, fail-open** lane it is the body of the
//!   `audit.v1.events` Kafka topic (~99% of traffic; Kafka is the durable buffer
//!   that decouples the business mesh from audit liveness). On the **synchronous,
//!   fail-closed** lane it rides inside `RecordPrivilegedRequest`.
//!
//! * **`RecordPrivileged` fails CLOSED.** It returns only once the event is
//!   durably persisted AND hash-chained — the response *is* the proof the caller
//!   needs to proceed. If durability is not confirmed within the deadline the RPC
//!   errors (`AUD-4004`) and the privileged action must be denied. The enrolled
//!   set is narrow and locked: break-glass access + legal-hold lifecycle.
//!
//! * **Reads are evidence too.** `Query` / `Export` / `VerifyIntegrity` are
//!   access-controlled (need-to-know + separation of duties) and each is itself
//!   recorded as an audit event. `Export` returns a *manifest* referencing a
//!   signed bundle in object storage — the bytes never travel on gRPC.
//!
//! Contract posture (the integrity guarantee): the ledger is append-only and
//! tamper-evident (per-partition hash chain + externally-anchored Merkle
//! checkpoints), and PII is carried only in the crypto-shreddable [`PiiEnvelope`]
//! (the chain hashes the ciphertext) so GDPR erasure = key destruction leaves the
//! record verifiable. The plane records decisions other services make; it makes
//! none and publishes nothing of record. See `project_audit_service_blueprint`.

tonic::include_proto!("audit.v1");

/// Encoded protobuf descriptor set for gRPC server reflection.
pub const FILE_DESCRIPTOR_SET: &[u8] =
    tonic::include_file_descriptor_set!("audit_descriptor");
