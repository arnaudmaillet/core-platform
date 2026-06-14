use crate::domain::entities::ProfileCounters;
use async_trait::async_trait;
use shared_kernel::core::Result;
use shared_kernel::types::ProfileId;

#[async_trait]
pub trait ProfileCountersIndexRepository: Send + Sync {
    async fn increment(&self, follower_id: ProfileId, following_id: ProfileId) -> Result<()>;
    async fn decrement(&self, follower_id: ProfileId, following_id: ProfileId) -> Result<()>;
    async fn read(&self, profile_id: ProfileId) -> Result<ProfileCounters>;
    async fn save(&self, counters: &ProfileCounters) -> Result<()>;
}
