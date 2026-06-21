// crates/profile/src/domain/repositories/profile_repository.rs

use crate::entities::Profile;
use async_trait::async_trait;
use shared_kernel::core::Result;
use shared_kernel::types::{AccountId, ProfileId};

#[async_trait]
pub trait ProfileRepository: Send + Sync {
    // Persiste ou met à jour le profil (gère en interne l'écriture双 (double) profiles + profiles_by_account)
    async fn save(&self, profile: &mut Profile) -> Result<()>;

    async fn find_by_id(&self, id: ProfileId) -> Result<Option<Profile>>;

    async fn find_all_by_account_id(&self, account_id: AccountId) -> Result<Vec<Profile>>;

    async fn delete(&self, id: ProfileId) -> Result<()>;

    async fn exists(&self, profile_id: ProfileId) -> Result<bool>;
}
