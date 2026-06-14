// crates/social/tests/stubs/counter.rs (ou ton dossier de test dédié)

use async_trait::async_trait;
use chrono::Utc;
use std::collections::{HashMap, HashSet};
use std::sync::RwLock;

use shared_kernel::core::{Error, Result};
use shared_kernel::types::{Counter, ProfileId};
use social::entities::ProfileCounters;
use social::repositories::{ProfileCountersIndexRepository, ProfileCountersStorageRepository};

pub struct CounterRepositoryStub {
    // Simule l'état en mémoire ou sur disque (valeurs absolues cumulées)
    storage: RwLock<HashMap<ProfileId, (u64, u64)>>,
    // Simule l'ensemble Redis "profiles:dirty"
    dirty_profiles: RwLock<HashSet<ProfileId>>,
}

impl CounterRepositoryStub {
    /// Crée un stub unifié capable de servir d'index chaud ou de stockage durable
    pub fn new() -> Self {
        Self {
            storage: RwLock::new(HashMap::new()),
            dirty_profiles: RwLock::new(HashSet::new()),
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

/// ---- 1. IMPLÉMENTATION DE L'INDEX CHAUD (COMPORTEMENT REDIS) ----
#[async_trait]
impl ProfileCountersIndexRepository for CounterRepositoryStub {
    async fn increment(&self, follower_id: ProfileId, following_id: ProfileId) -> Result<()> {
        let mut store = self.storage.write().unwrap();

        // Incrémente de façon atomique le cache chaud
        let follower_entry = store.entry(follower_id).or_insert((0, 0));
        follower_entry.1 = follower_entry.1.saturating_add(1);

        let following_entry = store.entry(following_id).or_insert((0, 0));
        following_entry.0 = following_entry.0.saturating_add(1);

        // Alimente systématiquement le tracking des profils modifiés
        let mut dirty = self.dirty_profiles.write().unwrap();
        dirty.insert(follower_id);
        dirty.insert(following_id);

        Ok(())
    }

    async fn decrement(&self, follower_id: ProfileId, following_id: ProfileId) -> Result<()> {
        let mut store = self.storage.write().unwrap();

        let follower_entry = store.entry(follower_id).or_insert((0, 0));
        follower_entry.1 = follower_entry.1.saturating_sub(1);

        let following_entry = store.entry(following_id).or_insert((0, 0));
        following_entry.0 = following_entry.0.saturating_sub(1);

        let mut dirty = self.dirty_profiles.write().unwrap();
        dirty.insert(follower_id);
        dirty.insert(following_id);

        Ok(())
    }

    async fn read(&self, profile_id: ProfileId) -> Result<ProfileCounters> {
        let store = self.storage.read().unwrap();

        // Comportement Cache-Aside : si absent du cache chaud, on lève une erreur NotFound
        match store.get(&profile_id) {
            Some(&(followers, following)) => Ok(ProfileCounters::restore(
                profile_id,
                Counter::from_raw(followers),
                Counter::from_raw(following),
                Utc::now(),
            )),
            None => Err(Error::not_found("ProfileCounters", profile_id.to_string())),
        }
    }

    async fn save(&self, counters: &ProfileCounters) -> Result<()> {
        let mut store = self.storage.write().unwrap();

        // Comportement Cache Warm-up : Écrasement par valeur absolue brute
        store.insert(
            counters.profile_id(),
            (
                counters.followers_count().value(),
                counters.following_count().value(),
            ),
        );
        Ok(())
    }
}

/// ---- 2. IMPLÉMENTATION DU STOCKAGE DISTRIBUÉ (COMPORTEMENT SCYLLADB) ----
#[async_trait]
impl ProfileCountersStorageRepository for CounterRepositoryStub {
    async fn commit_deltas(&self, counters: &ProfileCounters) -> Result<()> {
        let mut store = self.storage.write().unwrap();

        // Comportement ScyllaDB Natif : Application relative par delta incrémental
        let entry = store.entry(counters.profile_id()).or_insert((0, 0));
        entry.0 = entry.0.saturating_add(counters.followers_count().value());
        entry.1 = entry.1.saturating_add(counters.following_count().value());

        Ok(())
    }

    async fn fetch(&self, profile_id: ProfileId) -> Result<Option<ProfileCounters>> {
        let store = self.storage.read().unwrap();

        // Retourne un Option pour refléter fidèlement l'état de la base persistante à froid
        match store.get(&profile_id) {
            Some(&(followers, following)) => Ok(Some(ProfileCounters::restore(
                profile_id,
                Counter::from_raw(followers),
                Counter::from_raw(following),
                Utc::now(),
            ))),
            None => Ok(None),
        }
    }
}
