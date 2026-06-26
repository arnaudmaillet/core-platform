//! `audit-server` — the deployable binary for the audit **read/record plane**.
//!
//! Serves the internal gRPC plane via [`service_runtime::serve`] on :50068. Today
//! that is health + reflection only; Phase 5 wires the access-controlled
//! `Query` / `Export` / `VerifyIntegrity` reads and the synchronous, fail-closed
//! `RecordPrivileged` RPC. Scaled independently of `audit-worker`.

use audit::AuditServerService;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let addr = std::env::var("AUDIT_SERVER_GRPC_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:50068".to_owned())
        .parse()?;
    service_runtime::serve::<AuditServerService>(addr).await
}
