// crates/post/profile/src/domain/write_projection.rs

use crate::ProjectedProfile;
use async_trait::async_trait;
use shared_kernel::core::Result;

#[async_trait]
pub trait ProfileWriteProjection: Send + Sync {
    async fn save(&self, profile: &ProjectedProfile, updated_at_ms: i64) -> Result<()>;
}
