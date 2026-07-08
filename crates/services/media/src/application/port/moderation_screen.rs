//! The pre-publish moderation Screen gate (Plane C of `moderation`, called from
//! media's Plane B). media computes the content hash and asks moderation whether
//! the bytes are known-bad before the asset can go public. The concrete adapter is
//! a gRPC client to `moderation` (Phase 4).
//!
//! **Fail-closed:** a `blocked` decision quarantines the asset (and, for
//! catastrophic categories, places a legal hold); a `ScreenUnavailable` error must
//! NOT be folded into an allow — the caller treats an unavailable gate as a block
//! for CSAM-class media, never an optimistic publish.

use async_trait::async_trait;

use crate::domain::value_object::{AssetId, ContentHash, MediaKind, OwnerId};
use crate::error::MediaError;

/// The screen result. `blocked` means a known-bad match; `csam` flags a
/// catastrophic-category match that additionally warrants a legal hold (evidence
/// preservation). `Allow` is "no known-bad match", never "approved".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScreenDecision {
    pub blocked: bool,
    pub csam: bool,
    pub reference: Option<String>,
}

impl ScreenDecision {
    /// A clean screen — nothing matched.
    pub fn allow() -> Self {
        Self {
            blocked: false,
            csam: false,
            reference: None,
        }
    }
}

#[async_trait]
pub trait ModerationScreen: Send + Sync + 'static {
    async fn screen(
        &self,
        asset_id: &AssetId,
        owner_id: &OwnerId,
        content_hash: &ContentHash,
        kind: MediaKind,
    ) -> Result<ScreenDecision, MediaError>;
}
