use crate::domain::entities::ProfileCounters;
use async_trait::async_trait;
use shared_kernel::core::Result;
use shared_kernel::types::ProfileId;

#[async_trait]
pub trait ProfileCountersStorageRepository: Send + Sync {
    async fn commit_deltas(&self, counters: &ProfileCounters) -> Result<()>;
    async fn fetch(&self, profile_id: ProfileId) -> Result<Option<ProfileCounters>>;
}
