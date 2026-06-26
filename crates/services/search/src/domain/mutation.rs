//! The output of the projector: a single, engine-agnostic index mutation.
//!
//! The Phase-4 adapter translates each variant into a concrete OpenSearch
//! operation (`Upsert` → external-versioned index; `Delete` → delete-by-id;
//! `PurgeByAuthor` → delete-by-query; `SetSearchable` → versioned partial update).
//! [`IndexMutation::Skip`] carries *intentional no-ops* (a non-indexable entity, an
//! irrelevant event) so the consumer folds them into `Ok` and commits the offset
//! rather than dead-lettering — per the Consumer Runtime Standard.

use super::document::IndexDocument;
use super::value_object::{AuthorId, DocVersion, EntityKind, Searchable};

#[derive(Debug, Clone, PartialEq)]
pub enum IndexMutation {
    /// Index or replace the document at `version` (external versioning rejects a
    /// non-newer write at the engine — the idempotency guard).
    Upsert(IndexDocument),

    /// Flip the moderation-visibility flag on an existing document. Carries a
    /// `version` so a stale flip can't overwrite a newer state.
    SetSearchable {
        kind: EntityKind,
        id: String,
        searchable: Searchable,
        version: DocVersion,
    },

    /// Hard-delete a single document by id.
    Delete { kind: EntityKind, id: String },

    /// Deep GDPR purge of everything an actor authored, across all indices.
    PurgeByAuthor { author_id: AuthorId },

    /// An intentional no-op — commit the offset, index nothing.
    Skip(SkipReason),
}

/// Why a source event produced no index change. All are benign and committed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkipReason {
    /// The moderated entity does not map to any search index.
    NotIndexable,
}
