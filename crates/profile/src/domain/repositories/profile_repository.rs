// crates/profile/src/domain/repositories/profile_repository.rs

use crate::entities::Profile;
use crate::types::Handle;
use async_trait::async_trait;
use shared_kernel::core::{Result, Transaction};
use shared_kernel::types::{AccountId, ProfileId, Region};

#[async_trait]
#[async_trait]
pub trait ProfileRepository: Send + Sync {
    async fn save(
        &self,
        region: Region,
        profile: &mut Profile,
        tx: Option<&mut dyn Transaction>,
    ) -> Result<()>;

    async fn find_by_id(
        &self,
        id: ProfileId,
        region: Region,
        tx: Option<&mut dyn Transaction>,
    ) -> Result<Option<Profile>>;

    async fn find_by_handle(
        &self,
        handle: &Handle,
        region: Region,
        tx: Option<&mut dyn Transaction>,
    ) -> Result<Option<Profile>>;

    async fn find_all_by_account_id(
        &self,
        account_id: AccountId,
        tx: Option<&mut dyn Transaction>,
    ) -> Result<Vec<Profile>>;

    async fn delete(
        &self,
        id: ProfileId,
        region: Region,
        tx: Option<&mut dyn Transaction>,
    ) -> Result<()>;

    async fn exists(&self, profile_id: ProfileId, region: Region) -> Result<bool>;
    async fn exists_by_handle(&self, handle: &Handle, region: Region) -> Result<bool>;
}
