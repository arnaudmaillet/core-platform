use async_trait::async_trait;

use crate::domain::value_object::ProfileId;
use crate::error::PostError;

/// A denormalized `profile_id → author_tier` projection, maintained from
/// `profile.v1.events` (`ProfileTierChanged`) and read on the publish path to
/// stamp the author's current tier onto the published event. Keeping this local
/// avoids a synchronous call to `profile` when publishing.
#[async_trait]
pub trait AuthorTierStore: Send + Sync + 'static {
    /// The author's current tier (0=Standard, 1=Premium, 2=Vip). Defaults to
    /// `0` (Standard) for an author the projection hasn't seen a tier change for.
    async fn get_tier(&self, profile_id: &ProfileId) -> Result<u8, PostError>;

    /// Upsert an author's tier (idempotent, last-writer-wins).
    async fn upsert_tier(&self, profile_id: &ProfileId, tier: u8) -> Result<(), PostError>;
}
