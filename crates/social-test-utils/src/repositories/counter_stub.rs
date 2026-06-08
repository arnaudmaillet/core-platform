use async_trait::async_trait;
use chrono::Utc;
use std::collections::{HashMap, HashSet};
use std::sync::RwLock;

use shared_kernel::core::{Error, Result};
use shared_kernel::types::{Counter, ProfileId};
use social::entities::ProfileCounters;
use social::repositories::CounterRepository;

pub struct CounterRepositoryStub {
    // Simule la table ou le Hash des compteurs par profil
    // On stocke un tuple (followers, following) sous forme de u64
    storage: RwLock<HashMap<ProfileId, (u64, u64)>>,
    // Simule l'ensemble Redis "profiles:dirty" pour le suivi des synchronisations
    dirty_profiles: RwLock<HashSet<ProfileId>>,
    // Flag déterminant si le stub se comporte comme Redis (lève une erreur si absent)
    is_cache_behavior: bool,
}

impl CounterRepositoryStub {
    /// Crée un stub configuré au choix : comportement Cache (Redis) ou DB (Scylla)
    pub fn new(is_cache_behavior: bool) -> Self {
        Self {
            storage: RwLock::new(HashMap::new()),
            dirty_profiles: RwLock::new(HashSet::new()),
            is_cache_behavior,
        }
    }

    /// Permet d'injecter des données brutes en amont pour configurer le scénario de test (Given)
    pub fn seed_counters(&self, profile_id: ProfileId, followers: u64, following: u64) {
        let mut store = self.storage.write().unwrap();
        store.insert(profile_id, (followers, following));
    }

    /// Permet de vérifier l'état du set dirty dans les assertions de test (Then)
    pub fn is_profile_dirty(&self, profile_id: &ProfileId) -> bool {
        let dirty = self.dirty_profiles.read().unwrap();
        dirty.contains(profile_id)
    }
}

#[async_trait]
impl CounterRepository for CounterRepositoryStub {
    async fn increment_counters(
        &self,
        follower_id: ProfileId,
        following_id: ProfileId,
    ) -> Result<()> {
        let mut store = self.storage.write().unwrap();

        // Modifie ou initialise le follower (+1 following)
        let follower_entry = store.entry(follower_id).or_insert((0, 0));
        follower_entry.1 = follower_entry.1.saturating_add(1);

        // Modifie ou initialise le profil suivi (+1 follower)
        let following_entry = store.entry(following_id).or_insert((0, 0));
        following_entry.0 = following_entry.0.saturating_add(1);

        // Si comportement cache, on alimente le set dirty
        if self.is_cache_behavior {
            let mut dirty = self.dirty_profiles.write().unwrap();
            dirty.insert(follower_id);
            dirty.insert(following_id);
        }

        Ok(())
    }

    async fn decrement_counters(
        &self,
        follower_id: ProfileId,
        following_id: ProfileId,
    ) -> Result<()> {
        let mut store = self.storage.write().unwrap();

        // -1 following pour le follower
        let follower_entry = store.entry(follower_id).or_insert((0, 0));
        follower_entry.1 = follower_entry.1.saturating_sub(1);

        // -1 follower pour la personne suivie
        let following_entry = store.entry(following_id).or_insert((0, 0));
        following_entry.0 = following_entry.0.saturating_sub(1);

        if self.is_cache_behavior {
            let mut dirty = self.dirty_profiles.write().unwrap();
            dirty.insert(follower_id);
            dirty.insert(following_id);
        }

        Ok(())
    }

    async fn get_counters(&self, profile_id: ProfileId) -> Result<ProfileCounters> {
        let store = self.storage.read().unwrap();

        match store.get(&profile_id) {
            Some(&(followers, following)) => Ok(ProfileCounters::restore(
                profile_id,
                Counter::from_raw(followers),
                Counter::from_raw(following),
                Utc::now(),
            )),
            None => {
                if self.is_cache_behavior {
                    Err(Error::not_found("ProfileCounters", profile_id.to_string()))
                } else {
                    Ok(ProfileCounters::restore(
                        profile_id,
                        Counter::default(),
                        Counter::default(),
                        Utc::now(),
                    ))
                }
            }
        }
    }

    async fn save(&self, counters: &ProfileCounters) -> Result<()> {
        let mut store = self.storage.write().unwrap();

        if self.is_cache_behavior {
            store.insert(
                counters.profile_id(),
                (
                    counters.followers_count().value(),
                    counters.following_count().value(),
                ),
            );
        } else {
            let entry = store.entry(counters.profile_id()).or_insert((0, 0));
            entry.0 = entry.0.saturating_add(counters.followers_count().value());
            entry.1 = entry.1.saturating_add(counters.following_count().value());
        }

        Ok(())
    }
}
