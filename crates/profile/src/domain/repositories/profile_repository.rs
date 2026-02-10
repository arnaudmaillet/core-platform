// crates/profile/src/domain/repositories/profile_repository.rs

use async_trait::async_trait;
use shared_kernel::domain::transaction::Transaction;
use shared_kernel::domain::value_objects::RegionCode;
use shared_kernel::errors::Result;

use crate::domain::entities::Profile;
use crate::domain::value_objects::{Handle, ProfileId, ProfileStats};

#[async_trait]
pub trait ProfileRepository: Send + Sync {
    async fn assemble_full_profile(&self, id: &ProfileId, reg: &RegionCode, ) -> Result<Option<Profile>>;
    async fn resolve_profile_from_handle(&self, slug: &Handle, reg: &RegionCode, ) -> Result<Option<Profile>>;
    async fn fetch_identity_only(&self, id: &ProfileId, region: &RegionCode, ) -> Result<Option<Profile>>;
    async fn fetch_stats_only(&self, id: &ProfileId, region: &RegionCode, ) -> Result<Option<ProfileStats>>;
    async fn save_identity(&self, profile: &Profile, original: Option<&Profile>, tx: Option<&mut dyn Transaction>) -> Result<()>;
    async fn exists_by_handle(&self, handle: &Handle, reg: &RegionCode) -> Result<bool>;
    async fn delete_full_profile(&self, id: &ProfileId, region: &RegionCode) -> Result<()>;
}
