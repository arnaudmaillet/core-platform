// crates/post/src/application/repositories/profile_projection_repository.rs

use async_trait::async_trait;
use shared_kernel::core::Result;
use shared_kernel::types::ProfileId;
use shared_proto::profile::v1::ProfileSummaryDto;

#[async_trait]
pub trait ProfileProjectionRepository: Send + Sync {
    async fn save(&self, profile: &ProfileSummaryDto, updated_at_ms: i64) -> Result<()>;
    async fn find_by_id(&self, profile_id: &ProfileId) -> Result<Option<ProfileSummaryDto>>;
}
