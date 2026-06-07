// crates/profile/src/utils/test_utils.rs

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Mutex;

use profile::entities::Profile;
use profile::repositories::ProfileRepository;
use profile::types::Handle;
use shared_kernel::core::{ManagedEntity, Error, Result, Transaction, Versioned};
use shared_kernel::types::{AccountId, ProfileId, Region};

#[derive(Hash, Eq, PartialEq, Clone)]
pub(crate) struct ProfileKey {
    pub id: ProfileId,
    pub region: Region,
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
    pub async fn save_direct(&self, region: Region, profile: Profile) {
        let mut store = self.profiles.lock().unwrap();
        let key = ProfileKey {
            id: profile.profile_id(),
            region,
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
    pub async fn find_direct(&self, id: ProfileId) -> Option<Profile> {
        let store = self.profiles.lock().unwrap();
        // Comme on ne veut pas forcer la région dans l'assertion (souvent on teste justement si elle est bonne)
        // On cherche le profil par ID peu importe la région dans le stub
        store.values().find(|p| p.profile_id() == id).cloned()
    }

    /// Récupère un profil avec une clé précise
    pub async fn find_with_key_direct(&self, id: ProfileId, region: Region) -> Option<Profile> {
        let store = self.profiles.lock().unwrap();
        let key = ProfileKey { id, region };
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
    async fn save(
        &self,
        region: Region,
        profile: &mut Profile,
        _tx: Option<&mut dyn Transaction>,
    ) -> Result<()> {
        if let Some(err) = self.error_to_return.lock().unwrap().clone() {
            return Err(err);
        }

        let mut store = self.profiles.lock().unwrap();
        let key = ProfileKey {
            id: profile.profile_id(),
            region,
        };

        let next_version = profile.version();

        // 2. Logique de Concurrence Optimiste (OCC) - Strictement ISO Postgres
        if let Some(existing) = store.get(&key) {
            let current_db_version = existing.version();

            // 💡 FIX IDEMPOTENCE : C'est une écriture blanche technique si la version
            // est identique en DB et qu'aucun événement métier n'a été produit.
            let is_noop =
                next_version == current_db_version && profile.lifecycle().is_events_empty();

            if is_noop {
                // On court-circuite immédiatement l'écriture pour ne pas corrompre
                // l'état de l'entité ou du store de test.
                return Ok(());
            }

            // Validation de l'OCC standard si ce n'est pas un Noop
            if current_db_version != (next_version - 1) {
                return Err(Error::concurrency_conflict(format!(
                    "Stub OCC mismatch: DB v{}, App v{}",
                    current_db_version,
                    next_version - 1
                )));
            }
        }

        // 3. Persistance dans la map de test
        store.insert(key, profile.clone());

        Ok(())
    }

    async fn find_by_id(
        &self,
        id: ProfileId,
        region: Region,
        _tx: Option<&mut dyn Transaction>,
    ) -> Result<Option<Profile>> {
        let store = self.profiles.lock().unwrap();
        let key = ProfileKey { id, region };
        Ok(store.get(&key).cloned())
    }

    async fn find_by_handle(
        &self,
        handle: &Handle,
        region: Region,
        _tx: Option<&mut dyn Transaction>,
    ) -> Result<Option<Profile>> {
        if let Some(err) = self.error_to_return.lock().unwrap().clone() {
            return Err(err);
        }

        let store = self.profiles.lock().unwrap();

        let profile = store
            .iter()
            .find(|(key, p)| p.handle() == handle && key.region == region)
            .map(|(_, p)| p.clone());

        Ok(profile)
    }

    async fn find_all_by_account_id(
        &self,
        account_id: AccountId,
        region: Region,
        _tx: Option<&mut dyn Transaction>,
    ) -> Result<Vec<Profile>> {
        let store = self.profiles.lock().unwrap();
        let profiles: Vec<Profile> = store
            .iter()
            .filter(|(key, p)| p.account_id() == account_id && key.region == region)
            .map(|(_, p)| p.clone())
            .collect();

        Ok(profiles)
    }

    async fn delete(
        &self,
        id: ProfileId,
        region: Region,
        _tx: Option<&mut dyn Transaction>,
    ) -> Result<()> {
        let mut store = self.profiles.lock().unwrap();
        let key = ProfileKey { id, region };
        store.remove(&key);
        Ok(())
    }

    async fn exists(&self, id: ProfileId, region: Region) -> Result<bool> {
        if let Some(err) = self.error_to_return.lock().unwrap().clone() {
            return Err(err);
        }
        let store = self.profiles.lock().unwrap();
        let key = ProfileKey { id, region };

        Ok(store.contains_key(&key))
    }

    async fn exists_by_handle(&self, handle: &Handle, region: Region) -> Result<bool> {
        if let Some(err) = self.error_to_return.lock().unwrap().clone() {
            return Err(err);
        }

        let store = self.profiles.lock().unwrap();
        let exists = store
            .iter()
            .any(|(key, p)| p.handle() == handle && key.region == region);

        Ok(exists)
    }
}
