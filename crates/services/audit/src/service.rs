//! Adapts the audit composition roots to the fleet [`service_runtime::Service`]
//! contract — **twice**, because audit is two deployables that share a domain but
//! no process or failure domain:
//!
//! * [`AuditServerService`] (`audit-server`) — the read/record plane. Will serve
//!   the access-controlled `Query` / `Export` / `VerifyIntegrity` reads and the
//!   **synchronous, fail-closed** `RecordPrivileged` RPC on its internal gRPC port
//!   (:50068).
//! * [`AuditWorkerService`] (`audit-worker`) — the ingest/verify plane. Will run
//!   the supervised `run_consumer` ingestion lane (decode → chain → persist →
//!   archive) plus the verifier / checkpoint-anchor / retention / crypto-shred
//!   loops, and serves only health + reflection on its port.
//!
//! ## Phase 0
//! Both are **health-only no-op stubs** today: they build nothing, hold no
//! backend, register no domain RPC (the runtime adds the gRPC health service), and
//! are reported `SERVING` as soon as they are built. Phase 1 introduces the
//! `audit-api` contract (and reflection); Phases 3–5 wire the real adapters,
//! probes, the `RecordPrivileged` RPC, and the consumer/verify loops.

use std::sync::Arc;

use async_trait::async_trait;
use service_runtime::{InfraRegistry, Service};
use tonic::service::RoutesBuilder;

/// Readiness health-reporting key for both binaries. Mirrors the future
/// fully-qualified `audit.v1` gRPC service name the runtime marks `SERVING`.
const HEALTH_KEY: &str = "audit.v1.AuditService";

// ── Read / record server ────────────────────────────────────────────────────

/// The `audit-server` composition root — the read/record plane (:50068).
///
/// Phase 0: a no-op stub. Phase 5 wires the query/export read handlers and the
/// synchronous fail-closed `RecordPrivileged` RPC over the real ledger + key
/// adapters, plus the backend health probes.
#[derive(Debug, Default)]
pub struct AuditServerService;

#[async_trait]
impl Service for AuditServerService {
    const NAME: &'static str = "audit-server";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");
    const GRPC_SERVICE_NAME: &'static str = HEALTH_KEY;

    async fn build(_infra: Arc<InfraRegistry>) -> anyhow::Result<Self> {
        Ok(Self)
    }

    fn register(self, _routes: &mut RoutesBuilder) -> anyhow::Result<()> {
        // Phase 0: no domain RPC and no reflection yet (the `audit-api` descriptor
        // set arrives in Phase 1). The runtime registers the gRPC health service.
        Ok(())
    }
}

// ── Ingest / verify worker ──────────────────────────────────────────────────

/// The `audit-worker` composition root — the ingest/verify plane.
///
/// Phase 0: a no-op stub. Phase 5 spawns the supervised `run_consumer` ingestion
/// lane (decode → chain → persist → archive) and the verifier / checkpoint-anchor
/// / retention / crypto-shred loops; it exposes no domain RPC.
#[derive(Debug, Default)]
pub struct AuditWorkerService;

#[async_trait]
impl Service for AuditWorkerService {
    const NAME: &'static str = "audit-worker";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");
    const GRPC_SERVICE_NAME: &'static str = HEALTH_KEY;

    async fn build(_infra: Arc<InfraRegistry>) -> anyhow::Result<Self> {
        Ok(Self)
    }

    fn register(self, _routes: &mut RoutesBuilder) -> anyhow::Result<()> {
        Ok(())
    }
}
