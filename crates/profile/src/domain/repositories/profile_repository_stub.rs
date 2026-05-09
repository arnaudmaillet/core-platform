// crates/profile/src/utils/test_utils.rs

use async_trait::async_trait;
use shared_kernel::domain::entities::Versioned;
use std::collections::HashMap;
use std::sync::Mutex;

use crate::domain::entities::Profile;
use crate::domain::repositories::ProfileRepository;
use crate::domain::value_objects::{Handle, ProfileId};
use shared_kernel::domain::transaction::Transaction;
use shared_kernel::domain::value_objects::{AccountId, RegionCode};
use shared_kernel::errors::{DomainError, Result};

#[derive(Hash, Eq, PartialEq, Clone)]
struct ProfileKey {
    id: ProfileId,
    region: RegionCode,
}

pub struct ProfileRepositoryStub {
    pub profiles: Mutex<HashMap<ProfileKey, Profile>>,
    pub error_to_return: Mutex<Option<DomainError>>,
}

impl Default for ProfileRepositoryStub {
    fn default() -> Self {
        Self {
            profiles: Mutex::new(HashMap::new()),
            error_to_return: Mutex::new(None),
        }
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
                return Err(DomainError::ConcurrencyConflict {
                    reason: format!(
                        "Stub OCC mismatch: DB v{}, App v{}",
                        current_db_version,
                        next_version - 1
                    ),
                });
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
