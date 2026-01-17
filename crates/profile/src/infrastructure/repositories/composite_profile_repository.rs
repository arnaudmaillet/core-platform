// crates/profile/src/infrastructure/repositories/composite_profile_repository.rs

use async_trait::async_trait;
use std::sync::Arc;
use shared_kernel::domain::transaction::Transaction;
use shared_kernel::domain::value_objects::{RegionCode, AccountId, Username};
use shared_kernel::errors::Result;

use crate::domain::entities::Profile;
use crate::domain::repositories::{
    ProfileRepository,
    ProfileIdentityRepository,
    ProfileStatsRepository
};
use crate::domain::value_objects::ProfileStats;

/// Orchestrateur de persistence polyglotte.
/// Fusionne les données relationnelles (Postgres) et les compteurs (ScyllaDB).
pub struct CompositeProfileRepository {
    identity: Arc<dyn ProfileIdentityRepository>,
    stats: Arc<dyn ProfileStatsRepository>,
}

impl CompositeProfileRepository {
    pub fn new(
        identity: Arc<dyn ProfileIdentityRepository>,
        stats: Arc<dyn ProfileStatsRepository>,
    ) -> Self {
        Self { identity, stats }
    }

    fn merge_identity_and_stats(
        &self,
        profile_opt: Option<Profile>,
        stats_res: Result<Option<ProfileStats>>
    ) -> Result<Option<Profile>> {
        match profile_opt {
            Some(mut profile) => {
                if let Ok(Some(scylla_stats)) = stats_res {
                    profile.stats = scylla_stats;
                }
                Ok(Some(profile))
            },
            None => Ok(None),
        }
    }
}

#[async_trait]
impl ProfileRepository for CompositeProfileRepository {
    /// Méthode de fusion : Récupère l'identité et les stats en parallèle.
    async fn get_full_profile(&self, account_id: &AccountId, region: &RegionCode) -> Result<Option<Profile>> {
        // Exécution parallèle des deux requêtes IO pour minimiser la latence
        let (id_res, stats_res) = tokio::join!(
            self.identity.find_by_id(account_id, region),
            self.stats.find_by_id(account_id, region)
        );

        match id_res? {
            Some(mut profile) => {
                // Si Scylla répond, on injecte les compteurs réels.
                // Sinon (ex: Scylla temporairement down), on garde les stats par défaut (0).
                if let Ok(Some(scylla_stats)) = stats_res {
                    profile.stats = scylla_stats;
                }
                Ok(Some(profile))
            },
            None => Ok(None),
        }
    }

    async fn get_full_profile_by_username(&self, slug: &Username, reg: &RegionCode) -> Result<Option<Profile>> {
        // 1. On cherche d'abord l'identité par slug dans Postgres
        let id_opt = self.identity.find_by_username(slug, reg).await?;

        match id_opt {
            Some(profile) => {
                // 2. Si trouvé, on récupère les stats par ID dans Scylla
                let stats_res = self.stats.find_by_id(&profile.account_id, reg).await;
                self.merge_identity_and_stats(Some(profile), stats_res)
            },
            None => Ok(None)
        }
    }

    async fn get_profile_identity(&self, account_id: &AccountId, region: &RegionCode) -> Result<Option<Profile>> {
        self.identity.find_by_id(account_id, region).await
    }

    async fn get_profile_stats(&self, account_id: &AccountId, region: &RegionCode) -> Result<Option<ProfileStats>> {
        self.stats.find_by_id(account_id, region).await
    }
}

#[async_trait]
impl ProfileIdentityRepository for CompositeProfileRepository {
    async fn save(&self, profile: &Profile, tx: Option<&mut dyn Transaction>) -> Result<()> {
        self.identity.save(profile, tx).await
    }

    async fn find_by_id(&self, account_id: &AccountId, region: &RegionCode) -> Result<Option<Profile>> {
        self.identity.find_by_id(account_id, region).await
    }

    async fn find_by_username(&self, slug: &Username, region: &RegionCode) -> Result<Option<Profile>> {
        // Pour un find_by_slug, on veut souvent le profil complet
        let profile_opt = self.identity.find_by_username(slug, region).await?;

        match profile_opt {
            Some(mut profile) => {
                if let Ok(Some(s)) = self.stats.find_by_id(&profile.account_id, region).await {
                    profile.stats = s;
                }
                Ok(Some(profile))
            },
            None => Ok(None)
        }
    }

    async fn exists_by_username(&self, slug: &Username, region: &RegionCode) -> Result<bool> {
        self.identity.exists_by_username(slug, region).await
    }

    async fn delete_identity(&self, account_id: &AccountId, region: &RegionCode) -> Result<()> {
        self.identity.delete_identity(account_id, region).await
    }
}

#[async_trait]
impl ProfileStatsRepository for CompositeProfileRepository {
    async fn find_by_id(&self, account_id: &AccountId, region: &RegionCode) -> Result<Option<ProfileStats>> {
        self.stats.find_by_id(account_id, region).await
    }

    async fn update_stats(
        &self,
        account_id: &AccountId,
        region: &RegionCode,
        follower_delta: i64,
        following_delta: i64,
        post_delta: i64
    ) -> Result<()> {
        self.stats.update_stats(account_id, region, follower_delta, following_delta, post_delta).await
    }

    async fn delete_stats(&self, account_id: &AccountId, region: &RegionCode) -> Result<()> {
        self.stats.delete_stats(account_id, region).await
    }
}