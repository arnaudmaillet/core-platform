use crate::domain::value_objects::{ProfileId, ProfileStats};
use async_trait::async_trait;
use shared_kernel::domain::value_objects::RegionCode;
use shared_kernel::errors::Result;

#[async_trait]
pub trait ProfileStatsRepository: Send + Sync {
    async fn fetch(&self, id: &ProfileId, reg: &RegionCode) -> Result<Option<ProfileStats>>;
    async fn save(
        &self,
        profile_id: &ProfileId,
        reg: &RegionCode,
        follower_delta: i64,
        following_delta: i64,
        post_delta: i64,
    ) -> Result<()>;
    async fn delete(&self, profile_id: &ProfileId, reg: &RegionCode) -> Result<()>;
}
