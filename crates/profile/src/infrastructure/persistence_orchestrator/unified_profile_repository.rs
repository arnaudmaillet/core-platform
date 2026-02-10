// crates/profile/src/infrastructure/repositories/composite_profile_repository.rs

use async_trait::async_trait;
use shared_kernel::domain::transaction::Transaction;
use shared_kernel::domain::value_objects::{AccountId, RegionCode, Username};
use shared_kernel::errors::Result;
use std::sync::Arc;
use std::time::Duration;
use crate::domain::entities::Profile;
use crate::domain::repositories::{
    ProfileIdentityRepository, ProfileRepository, ProfileStatsRepository,
};
use crate::domain::value_objects::{Handle, ProfileId, ProfileStats};

/// Orchestrateur de persistence polyglotte.
/// Fusionne les données relationnelles (Postgres) et les compteurs (ScyllaDB).
pub struct UnifiedProfileRepository {
    identity: Arc<dyn ProfileIdentityRepository>,
    stats: Arc<dyn ProfileStatsRepository>,
    cache: Arc<dyn shared_kernel::domain::repositories::CacheRepository>,
}

impl UnifiedProfileRepository {
    pub fn new(
        identity: Arc<dyn ProfileIdentityRepository>,
        stats: Arc<dyn ProfileStatsRepository>,
        cache: Arc<dyn shared_kernel::domain::repositories::CacheRepository>,
    ) -> Self {
        Self { identity, stats, cache }
    }

    /// Logique de "Read-Through Cache" pour les statistiques
    async fn fetch_live_stats(&self, profile_id: &ProfileId) -> Result<ProfileStats> {
        let cache_key = format!("profile_stats:{}", profile_id);

        // 1. Tenter Redis (on ignore l'erreur Redis pour ne pas bloquer le flux)
        if let Ok(Some(json_str)) = self.cache.get(&cache_key).await {
            if let Ok(stats) = serde_json::from_str::<ProfileStats>(&json_str) {
                return Ok(stats);
            }
        }

        // 2. Fallback sur ScyllaDB (notre source de vérité)
        // Note: On utilise une région par défaut ou celle du contexte si nécessaire
        let scylla_stats = self.stats.fetch(profile_id, &RegionCode::from_raw("eu")).await?;
        let stats = scylla_stats.unwrap_or_default();

        // 3. Mettre à jour Redis en tâche de fond (Fire and forget)
        let cache_stats = stats.clone();
        let cache_repo = self.cache.clone();
        tokio::spawn(async move {
            if let Ok(json) = serde_json::to_string(&cache_stats) {
                let _ = cache_repo.set(&cache_key, &json, Some(Duration::from_secs(3600))).await;
            }
        });

        Ok(stats)
    }
}

#[async_trait]
impl ProfileRepository for UnifiedProfileRepository {
    /// Méthode de fusion : Récupère l'identité et les stats en parallèle.
    async fn assemble_full_profile(
        &self,
        id: &ProfileId,
        region: &RegionCode,
    ) -> Result<Option<Profile>> {
        // Exécution parallèle des deux requêtes IO pour minimiser la latence
        let (id_res, stats_res) = tokio::join!(
            self.identity.fetch(id, region),
            self.fetch_live_stats(id)
        );

        match id_res? {
            Some(mut profile) => {
                // On injecte les stats (soit Redis, soit Scylla, soit default)
                if let Ok(stats) = stats_res {
                    profile.restore_stats(stats);
                }
                Ok(Some(profile))
            }
            None => Ok(None),
        }
    }

    async fn resolve_profile_from_handle(
        &self,
        handle: &Handle,
        region: &RegionCode,
    ) -> Result<Option<Profile>> {
        let un_key = handle.as_str();
        let mapping_key = format!("un_to_id:{}", un_key);

        // 1. TENTATIVE "ELITE" : On cherche l'ID dans l'index Redis
        if let Ok(Some(id_str)) = self.cache.get(&mapping_key).await {
            if let Ok(profile_id) = ProfileId::try_new(&id_str) {
                return self.assemble_full_profile(&profile_id, region).await;
            }
        }

        // 2. FALLBACK : Si l'index n'existe pas, on passe par Postgres d'abord
        let profile_opt = self.identity.fetch_by_handle(handle, region).await?;

        match profile_opt {
            Some(mut profile) => {
                let profile_id = profile.id().clone();
                let stats = self.fetch_live_stats(&profile_id).await?;

                profile.restore_stats(stats);

                let _ = self.cache.set(
                    &mapping_key,
                    &profile_id.to_string(),
                    Some(Duration::from_secs(3600))
                ).await;

                Ok(Some(profile))
            }
            None => Ok(None),
        }
    }

    async fn fetch_identity_only(
        &self,
        id: &ProfileId,
        region: &RegionCode,
    ) -> Result<Option<Profile>> {
        self.identity.fetch(id, region).await
    }

    async fn fetch_stats_only(
        &self,
        id: &ProfileId,
        region: &RegionCode,
    ) -> Result<Option<ProfileStats>> {
        self.stats.fetch(id, region).await
    }

    // Dans CompositeProfileRepository
    async fn save_identity(&self, profile: &Profile, original: Option<&Profile>, tx: Option<&mut dyn Transaction>) -> Result<()> {
        // 1. Sauvegarde Postgres
        self.identity.save(profile, tx).await?;

        // 2. Invalidation intelligente
        if let Some(old) = original {
            if old.handle() != profile.handle() {
                // On supprime l'ancien index car le pseudo n'appartient plus à cet ID
                let _ = self.cache.delete(&format!("un_to_id:{}", old.handle().as_str())).await;
            }
        }

        // On invalide toujours le profil complet (stats + identité fusionnées) 
        // pour forcer le refresh au prochain assemble_full_profile
        let _ = self.cache.delete(&format!("profile_stats:{}", profile.id())).await;
        let _ = self.cache.delete(&format!("un_to_id:{}", profile.handle().as_str())).await;

        Ok(())
    }

    async fn exists_by_handle(&self, handle: &Handle, region: &RegionCode) -> Result<bool> {
        self.identity.exists_by_handle(handle, region).await
    }

    async fn delete_full_profile(&self, id: &ProfileId, region: &RegionCode) -> Result<()> {
        // On récupère le profile pour avoir le username (pour nettoyer l'index Redis)
        if let Ok(Some(profile)) = self.identity.fetch(id, region).await {
            let un_key = format!("idx:un_to_id:{}", profile.handle().as_str());
            let stats_key = format!("profile_stats:{}", id);

            let _ = self.cache.delete(&un_key).await;
            let _ = self.cache.delete(&stats_key).await;
        }

        // Suppression DB
        let _ = tokio::join!(
            self.identity.delete(id, region),
            self.stats.delete(id, region)
        );

        Ok(())
    }
}
