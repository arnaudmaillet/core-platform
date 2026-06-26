use async_trait::async_trait;

use crate::domain::{EntityKind, IndexDocument};
use crate::error::SearchError;

/// Reads the authoritative source-of-record to rebuild the index from truth.
///
/// Because Kafka retention is finite, "replay from earliest" is not a reliable
/// rebuild path — the index is reconstructed by scanning the SoR (the `post` /
/// `profile` services). The concrete implementation pages the source; this port is
/// the contract the [`Reindexer`](crate::application::reindex::Reindexer) drives.
/// (A concrete gRPC-backed source is a deferred follow-up — it needs the live
/// services and, for profiles, an upstream event/scan capability.)
#[async_trait]
pub trait BackfillSource: Send + Sync + 'static {
    /// Yield the current documents of `kind` from the source-of-record.
    async fn scan(&self, kind: EntityKind) -> Result<Vec<IndexDocument>, SearchError>;
}
