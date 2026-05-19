// crates/social/src/domain/repositories/counter.rs

use crate::domain::entities::ProfileCounters;
use async_trait::async_trait;
use shared_kernel::core::Result;
use shared_kernel::types::ProfileId;

#[async_trait]
pub trait CounterRepository: Send + Sync {
    /// Incrémente de manière atomique et thread-safe les compteurs d'un follow (+1 following pour A, +1 follower pour B).
    async fn increment_counters(
        &self,
        follower_id: ProfileId,
        following_id: ProfileId,
    ) -> Result<()>;

    /// Décrémente de manière atomique et thread-safe les compteurs d'un unfollow (-1 following pour A, -1 follower pour B).
    async fn decrement_counters(
        &self,
        follower_id: ProfileId,
        following_id: ProfileId,
    ) -> Result<()>;

    /// Récupère l'état consolidé des compteurs d'un profil donné (Source chaude Redis, fallback DB).
    async fn get_counters(&self, profile_id: ProfileId) -> Result<ProfileCounters>;

    /// Persiste ou synchronise l'état complet d'un agrégat de compteurs (Utile pour le Worker asynchrone de Write-Back).
    async fn save(&self, counters: &ProfileCounters) -> Result<()>;
}
