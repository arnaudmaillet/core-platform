// coreshared_kernel/src/domain/routing.rs

use async_trait::async_trait;
use shared_kernel::{
    core::Result,
    types::{ProfileId, Region},
};

#[async_trait]
pub trait ProfileRoutingRepository: Send + Sync {
    /// Retrouve la région d'un profil à partir de son ID ($O(1)$ global)
    async fn find_region_by_id(&self, profile_id: &ProfileId) -> Result<Option<Region>>;

    /// Retrouve l'ID et la région d'un profil à partir du hash de son slug ($O(1)$ global)
    async fn resolve_slug(&self, slug_hash: &str) -> Result<Option<(ProfileId, Region)>>;

    /// Enregistre le routage initial (ID et Slug) lors de la création d'un profil
    async fn register_routing(
        &self,
        profile_id: ProfileId,
        slug_hash: &str,
        region: Region,
    ) -> Result<()>;

    /// Met à jour de manière atomique et sécurisée le changement de handle
    async fn update_slug_routing(
        &self,
        profile_id: ProfileId,
        old_slug_hash: &str,
        new_slug_hash: &str,
        region: Region,
    ) -> Result<()>;

    /// Supprime proprement les entrées de routage (ex: suppression de compte)
    async fn delete_routing(&self, profile_id: ProfileId, slug_hash: &str) -> Result<()>;
}
