//! `audit-worker` — the deployable binary for the audit **ingest/verify plane**.
//!
//! Serves only the gRPC health/reflection plane via [`service_runtime::serve`] on
//! :50069 so Kubernetes can probe readiness; Phase 5 spawns the supervised
//! `run_consumer` ingestion lane (decode → chain → persist → archive) and the
//! verifier / checkpoint-anchor / retention / crypto-shred loops. It exposes no
//! domain RPC. Scaled independently of `audit-server`.

use audit::AuditWorkerService;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let addr = std::env::var("AUDIT_WORKER_GRPC_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:50069".to_owned())
        .parse()?;
    service_runtime::serve::<AuditWorkerService>(addr).await
}
