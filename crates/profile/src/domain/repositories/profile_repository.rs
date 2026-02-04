// crates/profile/src/domain/repositories/profile_repository.rs

use async_trait::async_trait;
use shared_kernel::domain::transaction::Transaction;
use shared_kernel::domain::value_objects::{AccountId, RegionCode, Username};
use shared_kernel::errors::Result;

use crate::domain::entities::Profile;
use crate::domain::value_objects::ProfileStats;

#[async_trait]
pub trait ProfileRepository: Send + Sync {
    async fn get_profile_by_account_id(
        &self,
        id: &AccountId,
        reg: &RegionCode,
    ) -> Result<Option<Profile>>;
    async fn get_full_profile_by_username(
        &self,
        slug: &Username,
        reg: &RegionCode,
    ) -> Result<Option<Profile>>;
    async fn get_profile_without_stats(
        &self,
        account_id: &AccountId,
        region: &RegionCode,
    ) -> Result<Option<Profile>>;
    async fn get_profile_stats(
        &self,
        account_id: &AccountId,
        region: &RegionCode,
    ) -> Result<Option<ProfileStats>>;

    async fn save(&self, profile: &Profile, tx: Option<&mut dyn Transaction>) -> Result<()>;
    async fn exists_by_username(&self, username: &Username, reg: &RegionCode) -> Result<bool>;
}
