// crates/profile/src/utils/test_utils.rs

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Mutex;

use profile_old::entities::Profile;
use profile_old::repositories::ProfileRepository;
use shared_kernel::core::{Error, Result, Versioned};
use shared_kernel::messaging::{Event, EventEmitter};
use shared_kernel::types::{AccountId, ProfileId};

#[derive(Hash, Eq, PartialEq, Clone)]
pub(crate) struct ProfileKey(pub ProfileId);

pub struct ProfileRepositoryStub {
    profiles: Mutex<HashMap<ProfileKey, Profile>>,
    captured_events: Mutex<HashMap<ProfileId, Vec<Box<dyn Event>>>>,
    error_to_return: Mutex<Option<Error>>,
}

impl Default for ProfileRepositoryStub {
    fn default() -> Self {
        Self {
            profiles: Mutex::new(HashMap::new()),
            captured_events: Mutex::new(HashMap::new()),
            error_to_return: Mutex::new(None),
        }
    }
}

impl ProfileRepositoryStub {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn save_direct(&self, profile: Profile) {
        let mut store = self.profiles.lock().unwrap();
        let key = ProfileKey(profile.profile_id());
        store.insert(key, profile);
    }

    pub async fn find_direct(&self, id: ProfileId) -> Option<Profile> {
        let store = self.profiles.lock().unwrap();
        store.values().find(|p| p.profile_id() == id).cloned()
    }
    pub async fn get_captured_events(&self, id: ProfileId) -> Vec<Box<dyn Event>> {
        let captured = self.captured_events.lock().unwrap();
        captured.get(&id).cloned().unwrap_or_default()
    }

    pub fn set_error(&self, err: Error) {
        *self.error_to_return.lock().unwrap() = Some(err);
    }

    pub fn clear(&self) {
        self.profiles.lock().unwrap().clear();
        self.captured_events.lock().unwrap().clear();
        *self.error_to_return.lock().unwrap() = None;
    }
}

#[async_trait]
impl ProfileRepository for ProfileRepositoryStub {
    async fn save(&self, profile: &mut Profile) -> Result<()> {
        if let Some(err) = self.error_to_return.lock().unwrap().clone() {
            return Err(err);
        }

        let mut store = self.profiles.lock().unwrap();
        let key = ProfileKey(profile.profile_id());

        if let Some(existing) = store.get(&key) {
            if existing.version() != profile.version() - 1 {
                return Err(Error::concurrency_conflict("OCC mismatch".to_string()));
            }
        }

        let events = profile.pull_events();
        if !events.is_empty() {
            let mut captured = self.captured_events.lock().unwrap();
            captured
                .entry(profile.profile_id())
                .or_default()
                .extend(events);
        }

        store.insert(key, profile.clone());
        Ok(())
    }

    async fn find_by_id(&self, id: ProfileId) -> Result<Option<Profile>> {
        let store = self.profiles.lock().unwrap();
        Ok(store.get(&ProfileKey(id)).cloned())
    }

    async fn find_all_by_account_id(&self, account_id: AccountId) -> Result<Vec<Profile>> {
        let store = self.profiles.lock().unwrap();
        Ok(store
            .values()
            .filter(|p| p.account_id() == account_id)
            .cloned()
            .collect())
    }

    async fn delete(&self, id: ProfileId) -> Result<()> {
        let mut store = self.profiles.lock().unwrap();
        store.retain(|_, p| p.profile_id() != id);
        Ok(())
    }

    async fn exists(&self, id: ProfileId) -> Result<bool> {
        let store = self.profiles.lock().unwrap();
        Ok(store.values().any(|p| p.profile_id() == id))
    }
}
