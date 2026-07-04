use async_trait::async_trait;

use crate::domain::EntityKind;
use crate::error::SearchError;

/// The index lifecycle / operations port — the contract behind the blue-green
/// reindex story (Phase 7).
///
/// Index mappings and analyzers are a *schema*: changing them requires building a
/// new physical index and atomically repointing an alias, never an in-place edit.
/// This port is defined now so the application owns the vocabulary; the OpenSearch
/// adapter (Phase 4) and the reindex/backfill job (Phase 7) implement it. No
/// request-path handler depends on it.
#[async_trait]
pub trait IndexAdmin: Send + Sync + 'static {
    /// Ensure every per-kind physical index and its read/write aliases exist with
    /// the current mapping. Idempotent; run at startup and before a rollout.
    async fn ensure_indices(&self) -> Result<(), SearchError>;

    /// Create a fresh physical index for `kind` (the target of a reindex), named
    /// with `suffix` (e.g. a version stamp), without touching live aliases. Returns
    /// the physical index name it created — naming stays the adapter's concern.
    async fn create_index_version(
        &self,
        kind: EntityKind,
        suffix: &str,
    ) -> Result<String, SearchError>;

    /// Atomically repoint the **write** alias for `kind` to `physical_index`, so
    /// live writes (and the backfill) land on the new index during a reindex.
    async fn swap_write_alias(
        &self,
        kind: EntityKind,
        physical_index: &str,
    ) -> Result<(), SearchError>;

    /// Atomically repoint the **read** alias for `kind` to `physical_index` — the
    /// zero-downtime cutover at the end of a reindex.
    async fn swap_read_alias(
        &self,
        kind: EntityKind,
        physical_index: &str,
    ) -> Result<(), SearchError>;
}
