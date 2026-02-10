// crates/profile/src/domain/repositories/profile_identity_repository.rs

use crate::domain::entities::Profile;
use async_trait::async_trait;
use shared_kernel::domain::transaction::Transaction;
use shared_kernel::domain::value_objects::{AccountId, RegionCode};
use shared_kernel::errors::Result;
use crate::domain::value_objects::{Handle, ProfileId};

#[async_trait]
pub trait ProfileIdentityRepository: Send + Sync {
    async fn save(&self, profile: &Profile, tx: Option<&mut dyn Transaction>) -> Result<()>;
    async fn fetch(&self, id: &ProfileId, region: &RegionCode, ) -> Result<Option<Profile>>;
    async fn fetch_by_handle(&self, handle: &Handle, region: &RegionCode, ) -> Result<Option<Profile>>;
    async fn fetch_all_by_owner(&self, owner_id: &AccountId) -> Result<Vec<Profile>>;
    async fn exists_by_handle(&self, handle: &Handle, region: &RegionCode) -> Result<bool>;
    async fn delete(&self, id: &ProfileId, region: &RegionCode) -> Result<()>;
}
