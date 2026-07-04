//! PostgreSQL adapter — the asset metadata System of Record.
//!
//! Like `account`/`auth`/`moderation`, writes use a single pool via
//! [`TransactionManager`]; true `owner_id` sharding is deferred (the column is
//! present so it can be added without a schema change). The rich [`Asset`]
//! aggregate is persisted as a JSONB `doc` (it derives `Serialize`/`Deserialize`,
//! with pending events `#[serde(skip)]`), so a row round-trips straight back to the
//! aggregate. Upserts are last-write-wins; an optimistic-lock tightening
//! (`ConcurrentModification`, MED-2003) is a later iteration.
//!
//! [`Asset`]: crate::domain::aggregate::Asset
//! [`TransactionManager`]: postgres_storage::TransactionManager

pub mod pg_asset_repository;

pub use pg_asset_repository::PgAssetRepository;

use postgres_storage::StorageError;

use crate::error::MediaError;

/// Maps a raw sqlx error into the media error namespace (delegating the code to the
/// shared `DB-*` storage taxonomy).
pub(crate) fn storage_err(e: sqlx::Error) -> MediaError {
    MediaError::Storage(StorageError::from(e))
}
