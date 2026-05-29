use async_trait::async_trait;
use chrono::Utc;
use std::collections::HashMap;
use std::sync::RwLock;

use shared_kernel::core::Result;
use shared_kernel::types::ProfileId;
use social::entities::FollowRelation;
use social::repositories::RelationRepository;

pub struct RelationRepositoryStub {
    followings: RwLock<HashMap<ProfileId, HashMap<ProfileId, FollowRelation>>>,
    followers: RwLock<HashMap<ProfileId, HashMap<ProfileId, chrono::DateTime<Utc>>>>,
}

impl RelationRepositoryStub {
    pub fn new() -> Self {
        Self {
            followings: RwLock::new(HashMap::new()),
            followers: RwLock::new(HashMap::new()),
        }
    }

    /// Permet de pré-alimenter le graphe social pour les scénarios de test (Given)
    pub fn seed_relation(&self, follower_id: ProfileId, following_id: ProfileId) {
        let relation =
            FollowRelation::restore(follower_id, following_id, 1, Utc::now(), Utc::now());

        let mut followings_map = self.followings.write().unwrap();
        followings_map
            .entry(follower_id)
            .or_default()
            .insert(following_id, relation);

        let mut followers_map = self.followers.write().unwrap();
        followers_map
            .entry(following_id)
            .or_default()
            .insert(follower_id, Utc::now());
    }
}

#[async_trait]
impl RelationRepository for RelationRepositoryStub {
    async fn save(&self, relation: &FollowRelation) -> Result<()> {
        let follower_id = *relation.follower_id();
        let following_id = *relation.following_id();

        // 1. Écriture dans la table principale (followings)
        let mut followings_map = self.followings.write().unwrap();
        followings_map
            .entry(follower_id)
            .or_default()
            .insert(following_id, relation.clone());

        // 2. Écriture synchrone dans la table miroir (followers)
        let mut followers_map = self.followers.write().unwrap();
        followers_map
            .entry(following_id)
            .or_default()
            .insert(follower_id, relation.created_at());

        Ok(())
    }

    async fn delete(&self, follower_id: ProfileId, following_id: ProfileId) -> Result<()> {
        // 1. Retrait de la table principale
        let mut followings_map = self.followings.write().unwrap();
        if let Some(map) = followings_map.get_mut(&follower_id) {
            map.remove(&following_id);
        }

        // 2. Retrait de la table miroir
        let mut followers_map = self.followers.write().unwrap();
        if let Some(map) = followers_map.get_mut(&following_id) {
            map.remove(&follower_id);
        }

        Ok(())
    }

    async fn find(
        &self,
        follower_id: ProfileId,
        following_id: ProfileId,
    ) -> Result<Option<FollowRelation>> {
        let followings_map = self.followings.read().unwrap();

        let relation = followings_map
            .get(&follower_id)
            .and_then(|map| map.get(&following_id))
            .cloned();

        Ok(relation)
    }

    async fn is_following(&self, follower_id: ProfileId, following_id: ProfileId) -> Result<bool> {
        let followings_map = self.followings.read().unwrap();

        let exists = followings_map
            .get(&follower_id)
            .map(|map| map.contains_key(&following_id))
            .unwrap_or(false);

        Ok(exists)
    }

    async fn get_following_ids(
        &self,
        follower_id: ProfileId,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<ProfileId>> {
        let followings_map = self.followings.read().unwrap();

        let Some(map) = followings_map.get(&follower_id) else {
            return Ok(Vec::new());
        };

        // Extraction, tri deterministe simulé par date ou ID, puis pagination
        let mut keys: Vec<ProfileId> = map.keys().cloned().collect();
        keys.sort(); // Tri basique pour garantir le déterminisme du test

        let paged = keys
            .into_iter()
            .skip(offset as usize)
            .take(limit as usize)
            .collect();

        Ok(paged)
    }

    async fn get_followers_ids(
        &self,
        following_id: ProfileId,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<ProfileId>> {
        let followers_map = self.followers.read().unwrap();

        let Some(map) = followers_map.get(&following_id) else {
            return Ok(Vec::new());
        };

        let mut keys: Vec<ProfileId> = map.keys().cloned().collect();
        keys.sort();

        let paged = keys
            .into_iter()
            .skip(offset as usize)
            .take(limit as usize)
            .collect();

        Ok(paged)
    }
}
