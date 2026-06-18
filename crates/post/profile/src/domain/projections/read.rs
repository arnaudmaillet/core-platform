// crates/post/profile/src/domain/read_projection.rs

use crate::ProjectedProfile;
use async_trait::async_trait;
use shared_kernel::core::Result;
use shared_kernel::types::ProfileId;

#[async_trait]
pub trait ProfileReadProjection: Send + Sync {
    async fn find_by_id(&self, profile_id: &ProfileId) -> Result<Option<ProjectedProfile>>;
}
