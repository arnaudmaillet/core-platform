use async_trait::async_trait;

use crate::domain::{
    AuthorId, DocVersion, EntityKind, IndexDocument, Searchable, SearchQuery, SearchResults,
    SuggestQuery, Suggestions,
};
use crate::error::SearchError;

/// The result of a version-guarded write. A `RejectedStale` is **not** an error —
/// it means the engine's external-version guard saw a newer revision already
/// stored and declined the write. The consumer commits the offset regardless.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteOutcome {
    Applied,
    RejectedStale,
}

/// The inverted index — the one port the read and write paths share.
///
/// Two independent version namespaces guard one document. `upsert` carries the
/// **content** version (from the source revision) and replaces the matchable /
/// display fields; it MUST preserve an already-stored moderation visibility (a
/// content re-projection never un-hides a moderated document). `set_searchable`
/// carries the **visibility** version (from the moderation event time) and updates
/// only the `searchable` flag. Keeping the two guards separate is what lets content
/// edits and moderation flips — which arrive on different topics with unrelated
/// version timelines — interleave correctly, in any order.
#[async_trait]
pub trait SearchIndex: Send + Sync + 'static {
    /// Index or replace a document's content, guarded by its content version.
    /// First index seeds visibility from `document.searchable()`; a re-index
    /// preserves the stored visibility.
    async fn upsert(&self, document: &IndexDocument) -> Result<WriteOutcome, SearchError>;

    /// Update only the moderation-visibility flag, guarded by the visibility
    /// version. If the document is not yet indexed (a hide that raced ahead of the
    /// content event), the intent is recorded so a later `upsert` honours it.
    async fn set_searchable(
        &self,
        kind: EntityKind,
        id: &str,
        searchable: Searchable,
        version: DocVersion,
    ) -> Result<WriteOutcome, SearchError>;

    /// Hard-delete a single document by id. Idempotent — deleting an absent id is a
    /// no-op success.
    async fn delete(&self, kind: EntityKind, id: &str) -> Result<(), SearchError>;

    /// GDPR deep purge: remove every document authored by `author_id` across all
    /// indices, with no retained tombstone. Returns the number removed.
    async fn purge_by_author(&self, author_id: &AuthorId) -> Result<u64, SearchError>;

    /// Run a federated query. Implementations fail OPEN — on partial/unavailable
    /// shards they return what they can with `SearchResults.degraded = true` rather
    /// than erroring.
    async fn search(&self, query: &SearchQuery) -> Result<SearchResults, SearchError>;

    /// Prefix autocomplete.
    async fn suggest(&self, query: &SuggestQuery) -> Result<Suggestions, SearchError>;
}
