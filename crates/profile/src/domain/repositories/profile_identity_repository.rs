// crates/profile/src/domain/repositories/profile_identity_repository.rs

use crate::domain::entities::Profile;
use async_trait::async_trait;
use shared_kernel::domain::transaction::Transaction;
use shared_kernel::domain::value_objects::{AccountId, RegionCode, Username};
use shared_kernel::errors::Result;

#[async_trait]
pub trait ProfileIdentityRepository: Send + Sync {
    async fn save(&self, profile: &Profile, tx: Option<&mut dyn Transaction>) -> Result<()>;
    async fn find_by_id(
        &self,
        account_id: &AccountId,
        region: &RegionCode,
    ) -> Result<Option<Profile>>;
    async fn find_by_username(
        &self,
        username: &Username,
        region: &RegionCode,
    ) -> Result<Option<Profile>>;
    async fn exists_by_username(&self, username: &Username, region: &RegionCode) -> Result<bool>;
    async fn delete_identity(&self, account_id: &AccountId, region: &RegionCode) -> Result<()>;
}
