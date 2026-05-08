// crates/profile/src/domain/repositories/profile_repository.rs

use crate::domain::entities::Profile;
use crate::domain::value_objects::{Handle, ProfileId};
use async_trait::async_trait;
use shared_kernel::domain::transaction::Transaction;
use shared_kernel::domain::value_objects::{AccountId, RegionCode};
use shared_kernel::errors::Result;

#[async_trait]
#[async_trait]
pub trait ProfileRepository: Send + Sync {
    async fn save(&self, profile: &mut Profile, tx: Option<&mut dyn Transaction>) -> Result<()>;

    async fn find_by_id(
        &self,
        id: &ProfileId,
        region: &RegionCode,
        tx: Option<&mut dyn Transaction>,
    ) -> Result<Option<Profile>>;

    async fn find_by_handle(
        &self,
        handle: &Handle,
        region: &RegionCode,
        tx: Option<&mut dyn Transaction>,
    ) -> Result<Option<Profile>>;

    async fn find_all_by_account_id(
        &self,
        account_id: &AccountId,
        tx: Option<&mut dyn Transaction>,
    ) -> Result<Vec<Profile>>;

    async fn delete(
        &self,
        id: &ProfileId,
        region: &RegionCode,
        tx: Option<&mut dyn Transaction>,
    ) -> Result<()>;
}
