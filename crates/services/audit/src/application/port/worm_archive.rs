use async_trait::async_trait;

use crate::domain::AuditRecord;
use crate::error::AuditError;

/// The long-term, write-once-read-many archive — the durability backstop beyond
/// the canonical ledger. The concrete adapter (Phase 4) is S3/MinIO Object Lock
/// in *compliance mode*, where not even the root account can delete or overwrite
/// an object before its retention expires.
///
/// Unreachability is `AUD-4002 ArchiveUnavailable` (retryable). Archiving lags the
/// ledger (the ledger is the synchronous source of truth); the worker reconciles
/// the archive forward.
#[async_trait]
pub trait WormArchive: Send + Sync + 'static {
    /// Persist a chained record to the immutable archive.
    async fn archive(&self, record: &AuditRecord) -> Result<(), AuditError>;

    /// Store a generated export bundle and return an opaque, access-controlled
    /// reference to it (NOT a URL with bytes — the caller resolves it out-of-band).
    async fn store_export(
        &self,
        export_id: &str,
        content: &[u8],
    ) -> Result<String, AuditError>;
}
