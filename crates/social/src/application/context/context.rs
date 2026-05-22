// crates/social/src/application/context/context.rs

use crate::domain::entities::{FollowRelation, ProfileCounters};
use crate::domain::repositories::{CounterRepository, RelationRepository};
use shared_kernel::core::ErrorCode;
use shared_kernel::{
    core::{Error, Result},
    idempotency::IdempotencyRepository,
    types::{ProfileId, Region},
};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct SocialAppContext {
    relation_repo: Arc<dyn RelationRepository>,
    cache_counter_repo: Arc<dyn CounterRepository>,
    counter_repo: Arc<dyn CounterRepository>,
    idempotency_repo: Arc<dyn IdempotencyRepository>,
}

impl SocialAppContext {
    pub fn new(
        relation_repo: Arc<dyn RelationRepository>,
        cache_counter_repo: Arc<dyn CounterRepository>,
        counter_repo: Arc<dyn CounterRepository>,
        idempotency_repo: Arc<dyn IdempotencyRepository>,
    ) -> Self {
        Self {
            relation_repo,
            cache_counter_repo,
            counter_repo,
            idempotency_repo,
        }
    }

    pub fn create_context(&self, target_profile_id: ProfileId, region: Region) -> SocialContext {
        SocialContext::new(self.clone(), target_profile_id, region)
    }
    pub fn relation_repo(&self) -> Arc<dyn RelationRepository> {
        self.relation_repo.clone()
    }
    pub fn cache_counter_repo(&self) -> Arc<dyn CounterRepository> {
        self.cache_counter_repo.clone()
    }
    pub fn counter_repo(&self) -> Arc<dyn CounterRepository> {
        self.counter_repo.clone()
    }
    pub fn idempotency_repo(&self) -> Arc<dyn IdempotencyRepository> {
        self.idempotency_repo.clone()
    }
}

#[derive(Clone)]
pub struct SocialContext {
    app: SocialAppContext,
    target_id: ProfileId,
    region: Region,
}

impl SocialContext {
    pub(crate) fn new(app: SocialAppContext, target_id: ProfileId, region: Region) -> Self {
        Self {
            app,
            target_id,
            region,
        }
    }

    pub fn region(&self) -> Region {
        self.region
    }

    pub async fn is_already_following(
        &self,
        follower_id: ProfileId,
        following_id: ProfileId,
    ) -> Result<bool> {
        self.app
            .relation_repo()
            .is_following(follower_id, following_id)
            .await
    }

    pub async fn get_following_list(
        &self,
        follower_id: ProfileId,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<ProfileId>> {
        self.app
            .relation_repo()
            .get_following_ids(follower_id, limit, offset)
            .await
    }

    /// Récupère la liste paginée des profils qui suivent un utilisateur
    pub async fn get_followers_list(
        &self,
        following_id: ProfileId,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<ProfileId>> {
        self.app
            .relation_repo()
            .get_followers_ids(following_id, limit, offset)
            .await
    }

    // --- BARRIÈRE D'IDEMPOTENCE À CHAUD ---
    pub async fn ensure_executable(&self, command_id: Uuid, region: &Region) -> Result<bool> {
        if region != &self.region {
            return Err(Error::validation(
                "region",
                "Region mismatch (sharding violation prevention)",
            ));
        }

        let exists = self
            .app
            .idempotency_repo()
            .exists(None, &command_id)
            .await?;

        if exists {
            return Ok(false);
        }
        Ok(true)
    }

    pub async fn get_profile_counters(&self, profile_id: ProfileId) -> Result<ProfileCounters> {
        // 1. Tentative de lecture sur le Hot Path Redis
        match self.app.cache_counter_repo().get_counters(profile_id).await {
            Ok(counters) => Ok(counters), // Cache Hit !

            Err(Error {
                code: ErrorCode::NotFound,
                ..
            }) => {
                // Cache Miss !
                // 2. Fallback : On va chercher la source de vérité dans ScyllaDB
                let db_counters = self.app.counter_repo().get_counters(profile_id).await?;

                // 3. Cache Warming : On ré-alimente Redis
                if let Err(e) = self.app.cache_counter_repo().save(&db_counters).await {
                    tracing::warn!(
                        "Failed to warm up Redis counter cache for {}: {:?}",
                        profile_id,
                        e
                    );
                }

                Ok(db_counters)
            }

            Err(other_error) => Err(other_error), // Si Redis a un autre problème (ex: Timeout), on remonte
        }
    }

    // --- ENREGISTREMENT SANS TRANSACTION DISTRIBUÉE ---
    pub async fn save_relation(
        &self,
        relation: &mut FollowRelation,
        command_id: Uuid,
    ) -> Result<()> {
        if self.region.as_static_str() != relation.follower_id().region_str() {
            return Err(Error::validation(
                "region",
                "Actor region mismatch violation",
            ));
        }

        // 1. Verrouillage / Sauvegarde de la commande dans Redis (SET NX)
        // Si la commande existe déjà, la méthode `save` de Redis lèvera directement un Error::already_exists
        self.app.idempotency_repo().save(None, &command_id).await?;

        // 2. L'écriture Graphe dans ScyllaDB (Synchrone & Immédiat)
        self.app.relation_repo().save(relation).await?;

        // 3. Le Hot Path Redis (Incréments Compteurs + Marquage Dirty)
        self.app
            .cache_counter_repo()
            .increment_counters(*relation.follower_id(), *relation.following_id())
            .await?;

        Ok(())
    }

    pub async fn delete_relation(
        &self,
        relation: &mut FollowRelation,
        command_id: Uuid,
    ) -> Result<()> {
        if self.region.as_static_str() != relation.follower_id().region_str() {
            return Err(Error::validation(
                "region",
                "Actor region mismatch violation",
            ));
        }

        // 1. Verrouillage / Sauvegarde de la commande de suppression dans Redis
        self.app.idempotency_repo().save(None, &command_id).await?;

        // 2. Suppression Graphe immédiate dans ScyllaDB
        self.app
            .relation_repo()
            .delete(*relation.follower_id(), *relation.following_id())
            .await?;

        // 3. Décrémentation atomique Redis
        self.app
            .cache_counter_repo()
            .decrement_counters(*relation.follower_id(), *relation.following_id())
            .await?;

        Ok(())
    }
}
