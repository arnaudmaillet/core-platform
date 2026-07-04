//! Domain events the media context publishes to the `media.v1.events` Kafka topic.
//!
//! Events are serde structs (JSON on the wire), matching the fleet convention —
//! they are deliberately **not** proto messages (the proto contract is the
//! synchronous RPC surface only). Every event carries `asset_id`, which the
//! infrastructure publisher (Phase 4) uses as the partition key so all events for
//! one asset stay ordered — `AssetReady` can never be delivered ahead of the
//! `AssetUploaded` it follows, and `AssetDeleted` is always last. `owner_id` rides
//! along as a secondary routing key.
//!
//! This is the decoupling feed that lets the publish path never wait on media:
//! `post`/`profile` reference a still-processing asset and react to `AssetReady`
//! (or lazily resolve at read); `moderation` screens off `AssetUploaded`; `search`
//! and GC consume readiness/lifecycle.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::value_object::{AssetId, ContentHash, MediaKind, OwnerId, RenditionKind};

/// An upload was finalized and validated; the bytes are in object storage. The
/// pipeline trigger and the moderation-screen input (carries the content hash).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssetUploaded {
    pub asset_id: AssetId,
    pub owner_id: OwnerId,
    pub kind: MediaKind,
    pub content_hash: ContentHash,
    pub byte_size: u64,
    pub occurred_at: DateTime<Utc>,
}

/// A single rendition became available — the progressive-render signal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssetVariantReady {
    pub asset_id: AssetId,
    pub owner_id: OwnerId,
    pub rendition: RenditionKind,
    pub occurred_at: DateTime<Utc>,
}

/// All renditions are available; the asset is deliverable. The signal `post`/
/// `profile`/`search` swap a placeholder for the real media on.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssetReady {
    pub asset_id: AssetId,
    pub owner_id: OwnerId,
    pub occurred_at: DateTime<Utc>,
}

/// Processing failed terminally (corrupt / unsupported / pipeline error).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssetFailed {
    pub asset_id: AssetId,
    pub owner_id: OwnerId,
    pub reason: String,
    pub occurred_at: DateTime<Utc>,
}

/// Delivery was revoked by a moderation takedown / compliance hold.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssetQuarantined {
    pub asset_id: AssetId,
    pub owner_id: OwnerId,
    pub occurred_at: DateTime<Utc>,
}

/// A quarantined asset was reinstated (moderation reversal / appeal).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssetRestored {
    pub asset_id: AssetId,
    pub owner_id: OwnerId,
    pub occurred_at: DateTime<Utc>,
}

/// The asset was hard-deleted (bytes purged). Consumers drop their references.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssetDeleted {
    pub asset_id: AssetId,
    pub owner_id: OwnerId,
    pub occurred_at: DateTime<Utc>,
}

/// Sealed sum type of every domain event media publishes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DomainEvent {
    AssetUploaded(AssetUploaded),
    AssetVariantReady(AssetVariantReady),
    AssetReady(AssetReady),
    AssetFailed(AssetFailed),
    AssetQuarantined(AssetQuarantined),
    AssetRestored(AssetRestored),
    AssetDeleted(AssetDeleted),
}

impl DomainEvent {
    /// Dotted routing key used as the Kafka message type header.
    pub fn event_type(&self) -> &'static str {
        match self {
            Self::AssetUploaded(_) => "media.asset_uploaded",
            Self::AssetVariantReady(_) => "media.asset_variant_ready",
            Self::AssetReady(_) => "media.asset_ready",
            Self::AssetFailed(_) => "media.asset_failed",
            Self::AssetQuarantined(_) => "media.asset_quarantined",
            Self::AssetRestored(_) => "media.asset_restored",
            Self::AssetDeleted(_) => "media.asset_deleted",
        }
    }

    /// The asset this event concerns — the Kafka partition key, guaranteeing
    /// per-asset ordering across the whole lifecycle.
    pub fn asset_id(&self) -> AssetId {
        match self {
            Self::AssetUploaded(e) => e.asset_id,
            Self::AssetVariantReady(e) => e.asset_id,
            Self::AssetReady(e) => e.asset_id,
            Self::AssetFailed(e) => e.asset_id,
            Self::AssetQuarantined(e) => e.asset_id,
            Self::AssetRestored(e) => e.asset_id,
            Self::AssetDeleted(e) => e.asset_id,
        }
    }

    /// The owning account (secondary routing key).
    pub fn owner_id(&self) -> OwnerId {
        match self {
            Self::AssetUploaded(e) => e.owner_id,
            Self::AssetVariantReady(e) => e.owner_id,
            Self::AssetReady(e) => e.owner_id,
            Self::AssetFailed(e) => e.owner_id,
            Self::AssetQuarantined(e) => e.owner_id,
            Self::AssetRestored(e) => e.owner_id,
            Self::AssetDeleted(e) => e.owner_id,
        }
    }
}
