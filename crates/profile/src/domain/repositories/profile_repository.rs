// crates/profile/src/domain/repositories/profile_repository.rs

use async_trait::async_trait;
use shared_kernel::domain::value_objects::{RegionCode, AccountId, Username};
use shared_kernel::errors::Result;

use crate::domain::entities::Profile;
use crate::domain::repositories::{ProfileIdentityRepository, ProfileStatsRepository};
use crate::domain::value_objects::ProfileStats;

#[async_trait]
pub trait ProfileRepository: ProfileIdentityRepository + ProfileStatsRepository {
    async fn get_full_profile(&self, id: &AccountId, reg: &RegionCode) -> Result<Option<Profile>>;
    async fn get_full_profile_by_username(&self, slug: &Username, reg: &RegionCode) -> Result<Option<Profile>>;
    async fn get_profile_identity(&self, account_id: &AccountId, region: &RegionCode) -> Result<Option<Profile>>;
    async fn get_profile_stats(&self, account_id: &AccountId, region: &RegionCode) -> Result<Option<ProfileStats>>;
}