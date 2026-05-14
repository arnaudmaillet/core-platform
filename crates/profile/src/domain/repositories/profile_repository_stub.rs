// crates/profile/src/utils/test_utils.rs

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Mutex;

use crate::entities::Profile;
use crate::repositories::ProfileRepository;
use crate::types::{Handle, ProfileId};
use shared_kernel::core::{Error, Result, Transaction, Versioned};
use shared_kernel::types::{AccountId, RegionCode};

#[derive(Hash, Eq, PartialEq, Clone)]
pub(crate) struct ProfileKey {
    pub id: ProfileId,
    pub region: RegionCode,
}

pub struct ProfileRepositoryStub {
    profiles: Mutex<HashMap<ProfileKey, Profile>>,
    error_to_return: Mutex<Option<Error>>,
}

impl Default for ProfileRepositoryStub {
    fn default() -> Self {
        Self {
            profiles: Mutex::new(HashMap::new()),
            error_to_return: Mutex::new(None),
        }
    }
}

// Dans crates/profile/src/utils/test_utils.rs (ou là où se trouve ton stub)

impl ProfileRepositoryStub {
    pub fn new() -> Self {
        Self {
            profiles: Mutex::new(HashMap::new()),
            error_to_return: Mutex::new(None),
        }
    }

    // --- Helpers de Configuration (Arrange) ---

    /// Insère un profil directement sans vérifier l'OCC ou les erreurs forcées
    pub async fn save_direct(&self, profile: Profile) {
        let mut store = self.profiles.lock().unwrap();
        let key = ProfileKey {
            id: profile.profile_id().clone(),
            region: profile.account_id().region().clone(),
        };
        store.insert(key, profile);
    }

    /// Force une erreur pour le prochain appel au repository
    pub fn set_error(&self, err: Error) {
        let mut slot = self.error_to_return.lock().unwrap();
        *slot = Some(err);
    }

    // --- Helpers d'Assertion (Assert) ---

    /// Récupère un profil sans passer par le Result/async du trait
    pub async fn find_direct(&self, id: &ProfileId) -> Option<Profile> {
        let store = self.profiles.lock().unwrap();
        // Comme on ne veut pas forcer la région dans l'assertion (souvent on teste justement si elle est bonne)
        // On cherche le profil par ID peu importe la région dans le stub
        store.values().find(|p| p.profile_id() == id).cloned()
    }

    /// Récupère un profil avec une clé précise
    pub async fn find_with_key_direct(
        &self,
        id: &ProfileId,
        region: &RegionCode,
    ) -> Option<Profile> {
        let store = self.profiles.lock().unwrap();
        let key = ProfileKey {
            id: id.clone(),
            region: region.clone(),
        };
        store.get(&key).cloned()
    }

    pub fn count(&self) -> usize {
        self.profiles.lock().unwrap().len()
    }

    /// Vide le repository
    pub fn clear(&self) {
        self.profiles.lock().unwrap().clear();
        let mut slot = self.error_to_return.lock().unwrap();
        *slot = None;
    }
}

#[async_trait]
impl ProfileRepository for ProfileRepositoryStub {
    async fn save(&self, profile: &mut Profile, _tx: Option<&mut dyn Transaction>) -> Result<()> {
        // 1. Simulation d'erreur forcée (pour tester les cas limites)
        if let Some(err) = self.error_to_return.lock().unwrap().clone() {
            return Err(err);
        }

        let mut store = self.profiles.lock().unwrap();
        let key = ProfileKey {
            id: profile.profile_id().clone(),
            region: profile.account_id().region().clone(),
        };

        // 2. Logique de Concurrence Optimiste (OCC) - Strictement identique à Postgres
        if let Some(existing) = store.get(&key) {
            let next_version = profile.version();
            let current_db_version = existing.version();

            if current_db_version != (next_version - 1) {
                return Err(Error::concurrency_conflict(format!(
                    "Stub OCC mismatch: DB v{}, App v{}",
                    current_db_version,
                    next_version - 1
                )));
            }
        }

        store.insert(key, profile.clone());
        Ok(())
    }

    async fn find_by_id(
        &self,
        id: &ProfileId,
        region: &RegionCode,
        _tx: Option<&mut dyn Transaction>,
    ) -> Result<Option<Profile>> {
        let store = self.profiles.lock().unwrap();
        let key = ProfileKey {
            id: id.clone(),
            region: region.clone(),
        };
        Ok(store.get(&key).cloned())
    }

    async fn find_by_handle(
        &self,
        handle: &Handle,
        region: &RegionCode,
        _tx: Option<&mut dyn Transaction>,
    ) -> Result<Option<Profile>> {
        let store = self.profiles.lock().unwrap();
        // Simulation d'un scan de table avec respect de la région
        let profile = store
            .values()
            .find(|p| p.handle() == handle && p.account_id().region() == region)
            .cloned();
        Ok(profile)
    }

    async fn find_all_by_account_id(
        &self,
        account_id: &AccountId,
        _tx: Option<&mut dyn Transaction>,
    ) -> Result<Vec<Profile>> {
        let store = self.profiles.lock().unwrap();
        // Un compte peut avoir plusieurs profils, on filtre par AccountId
        let profiles: Vec<Profile> = store
            .values()
            .filter(|p| p.account_id() == account_id)
            .cloned()
            .collect();
        Ok(profiles)
    }

    async fn delete(
        &self,
        id: &ProfileId,
        region: &RegionCode,
        _tx: Option<&mut dyn Transaction>,
    ) -> Result<()> {
        let mut store = self.profiles.lock().unwrap();
        let key = ProfileKey {
            id: id.clone(),
            region: region.clone(),
        };
        store.remove(&key);
        Ok(())
    }
}
