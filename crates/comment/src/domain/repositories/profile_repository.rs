// crates/comment/src/domain/repositories/profile.rs

use crate::types::CommentUserProfile;
use async_trait::async_trait;
use shared_kernel::core::Result;
use shared_kernel::types::ProfileId;
use std::collections::HashMap;

#[async_trait]
pub trait CommentUserProfileRepository: Send + Sync {
    /// Insère ou met à jour un profil utilisateur dans la table de projection ScyllaDB.
    /// Utilisé lors de la réception d'un événement Kafka ou d'un rafraîchissement.
    async fn save(&self, profile: &CommentUserProfile) -> Result<()>;

    /// Sauvegarde un lot de profils en une seule opération.
    /// Crucial pour optimiser les performances du mécanisme de Lazy Loading / Backfill.
    async fn save_batch(&self, profiles: Vec<CommentUserProfile>) -> Result<()>;

    /// Récupère un ensemble de profils à partir d'une liste d'identifiants uniques.
    /// Retourne une HashMap associant chaque ProfileId à son profil pour une jointure en O(1).
    async fn find_batch(
        &self,
        profile_ids: &[ProfileId],
    ) -> Result<HashMap<ProfileId, CommentUserProfile>>;
}
