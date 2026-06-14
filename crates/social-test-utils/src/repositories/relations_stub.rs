// crates/social-test-utils/src/repositories.rs

use async_trait::async_trait;
use chrono::Utc;
use std::collections::HashMap;
use std::sync::RwLock;

use shared_kernel::core::Result;
use shared_kernel::messaging::{Event, EventEmitter};
use shared_kernel::types::ProfileId;
use social::entities::FollowRelation;
use social::repositories::FollowRelationRepository;

pub struct RelationRepositoryStub {
    followings: RwLock<HashMap<ProfileId, HashMap<ProfileId, FollowRelation>>>,
    followers: RwLock<HashMap<ProfileId, HashMap<ProfileId, chrono::DateTime<Utc>>>>,
    captured_events: RwLock<HashMap<ProfileId, Vec<Box<dyn Event>>>>,
}

impl RelationRepositoryStub {
    pub fn new() -> Self {
        Self {
            followings: RwLock::new(HashMap::new()),
            followers: RwLock::new(HashMap::new()),
            captured_events: RwLock::new(HashMap::new()),
        }
    }

    pub fn capture_events(&self, follower_id: ProfileId, events: Vec<Box<dyn Event>>) {
        if !events.is_empty() {
            let mut captured = self.captured_events.write().unwrap();
            captured.entry(follower_id).or_default().extend(events);
        }
    }

    /// Récupère les événements capturés spécifiques à un profil
    pub fn get_captured_events_for(&self, id: ProfileId) -> Vec<Box<dyn Event>> {
        let captured = self.captured_events.read().unwrap();
        captured.get(&id).cloned().unwrap_or_default()
    }

    pub fn clear(&self) {
        self.followings.write().unwrap().clear();
        self.followers.write().unwrap().clear();
        self.captured_events.write().unwrap().clear();
    }

    pub fn seed_relation(&self, follower_id: ProfileId, following_id: ProfileId) {
        let relation = FollowRelation::restore(follower_id, following_id, Utc::now(), Utc::now());

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
impl FollowRelationRepository for RelationRepositoryStub {
    async fn save(&self, relation: &mut FollowRelation) -> Result<()> {
        let follower_id = relation.follower_id();
        let following_id = relation.following_id();

        let mut followings_map = self.followings.write().unwrap();
        followings_map
            .entry(follower_id)
            .or_default()
            .insert(following_id, relation.clone());

        let mut followers_map = self.followers.write().unwrap();
        followers_map
            .entry(following_id)
            .or_default()
            .insert(follower_id, relation.created_at());

        let domain_events = relation.pull_events();
        if !domain_events.is_empty() {
            let mut captured = self.captured_events.write().unwrap();
            captured
                .entry(follower_id)
                .or_default()
                .extend(domain_events);
        }

        Ok(())
    }

    async fn delete(&self, relation: &mut FollowRelation) -> Result<()> {
        let follower_id = relation.follower_id();
        let following_id = relation.following_id();

        let mut followings_map = self.followings.write().unwrap();
        if let Some(map) = followings_map.get_mut(&follower_id) {
            map.remove(&following_id);
        }

        let mut followers_map = self.followers.write().unwrap();
        if let Some(map) = followers_map.get_mut(&following_id) {
            map.remove(&follower_id);
        }

        let domain_events = relation.pull_events();
        if !domain_events.is_empty() {
            let mut captured = self.captured_events.write().unwrap();
            captured
                .entry(follower_id)
                .or_default()
                .extend(domain_events);
        }

        Ok(())
    }

    async fn find(
        &self,
        follower_id: ProfileId,
        following_id: ProfileId,
    ) -> Result<Option<FollowRelation>> {
        let followings_map = self.followings.read().unwrap();
        Ok(followings_map
            .get(&follower_id)
            .and_then(|map| map.get(&following_id))
            .cloned())
    }

    async fn is_following(&self, follower_id: ProfileId, following_id: ProfileId) -> Result<bool> {
        let followings_map = self.followings.read().unwrap();
        Ok(followings_map
            .get(&follower_id)
            .map(|map| map.contains_key(&following_id))
            .unwrap_or(false))
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
        let mut keys: Vec<ProfileId> = map.keys().cloned().collect();
        keys.sort();
        Ok(keys
            .into_iter()
            .skip(offset as usize)
            .take(limit as usize)
            .collect())
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
        Ok(keys
            .into_iter()
            .skip(offset as usize)
            .take(limit as usize)
            .collect())
    }
}
