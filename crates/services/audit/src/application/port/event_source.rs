use async_trait::async_trait;

use crate::domain::AuditEvent;
use crate::error::AuditError;

/// The asynchronous ingestion feed — the stream of already-decoded
/// [`AuditEvent`]s the ingest loop chains. In production (Phase 4/5) this is the
/// Kafka `run_consumer` pipeline over `audit.v1.events` (and the decision streams
/// from `moderation` / `auth` / `account`) plus the decode layer that turns a raw
/// record into an `AuditEvent`. An in-memory fake backs the unit tests.
///
/// `next_event` yields `None` when the feed is drained (shutdown / a finite test
/// fixture). A decode fault surfaces as `AUD-8001`; a transient consume fault as
/// `AUD-8003`, so the loop can apply the runtime's retry/DLQ policy.
#[async_trait]
pub trait EventSource: Send + Sync + 'static {
    async fn next_event(&self) -> Result<Option<AuditEvent>, AuditError>;
}
