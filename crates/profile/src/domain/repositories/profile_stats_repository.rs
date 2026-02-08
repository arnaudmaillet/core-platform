use crate::domain::value_objects::ProfileStats;
use async_trait::async_trait;
use shared_kernel::domain::value_objects::{AccountId, RegionCode};
use shared_kernel::errors::Result;

#[async_trait]
pub trait ProfileStatsRepository: Send + Sync {
    async fn fetch(&self, id: &AccountId, reg: &RegionCode) -> Result<Option<ProfileStats>>;
    async fn save(
        &self,
        id: &AccountId,
        reg: &RegionCode,
        follower_delta: i64,
        following_delta: i64,
        post_delta: i64,
    ) -> Result<()>;
    async fn delete(&self, id: &AccountId, reg: &RegionCode) -> Result<()>;
}
