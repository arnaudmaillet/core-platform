// crates/social/src/domain/repositories/relations.rs

use crate::domain::entities::FollowRelation;
use async_trait::async_trait;
use shared_kernel::core::Result;
use shared_kernel::types::ProfileId;

#[async_trait]
pub trait RelationRepository: Send + Sync {
    /// Persiste une nouvelle relation de suivi ou met à jour une relation existante.
    async fn save(&self, relation: &mut FollowRelation) -> Result<()>;

    /// Supprime définitivement la relation entre un follower et un following.
    async fn delete(&self, relation: &mut FollowRelation) -> Result<()>;

    /// Récupère une relation spécifique pour vérification ou reconstruction (Pattern Restore).
    async fn find(
        &self,
        follower_id: ProfileId,
        following_id: ProfileId,
    ) -> Result<Option<FollowRelation>>;

    /// Vérifie rapidement l'existence d'un lien (Idéal pour l'affichage du bouton Follow/Unfollow).
    async fn is_following(&self, follower_id: ProfileId, following_id: ProfileId) -> Result<bool>;

    /// Récupère la liste paginée des IDs des profils suivis par cet utilisateur (Pour la construction du Feed).
    async fn get_following_ids(
        &self,
        follower_id: ProfileId,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<ProfileId>>;

    /// Récupère la liste paginée des IDs des profils qui suivent cet utilisateur (Pour les notifications/visibilité).
    async fn get_followers_ids(
        &self,
        following_id: ProfileId,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<ProfileId>>;
}
