// crates/profile/src/infrastructure/scylla/stores/routing_store_stub.rs (ou dans test_utils.rs)

use async_trait::async_trait;
use profile_old::repositories::ProfileRoutingRepository;
use shared_kernel::core::{Error, Result};
use shared_kernel::types::{ProfileId, Region};
use std::collections::HashMap;
use std::sync::Mutex;

pub struct ProfileRoutingRepositoryStub {
    slugs: Mutex<HashMap<String, (ProfileId, Region)>>,
    profiles: Mutex<HashMap<ProfileId, Region>>,
}

impl ProfileRoutingRepositoryStub {
    pub fn new() -> Self {
        Self {
            slugs: Mutex::new(HashMap::new()),
            profiles: Mutex::new(HashMap::new()),
        }
    }
}

#[async_trait]
impl ProfileRoutingRepository for ProfileRoutingRepositoryStub {
    async fn find_region_by_id(&self, profile_id: &ProfileId) -> Result<Option<Region>> {
        let profiles = self.profiles.lock().unwrap();
        Ok(profiles.get(profile_id).cloned())
    }

    async fn resolve_slug(&self, slug_hash: &str) -> Result<Option<(ProfileId, Region)>> {
        let slugs = self.slugs.lock().unwrap();
        Ok(slugs.get(slug_hash).cloned())
    }

    async fn register_routing(
        &self,
        profile_id: ProfileId,
        slug_hash: &str,
        region: Region,
    ) -> Result<()> {
        let mut slugs = self.slugs.lock().unwrap();
        let mut profiles = self.profiles.lock().unwrap();

        // Simulation de la LWT (Lightweight Transaction)
        if slugs.contains_key(slug_hash) {
            return Err(Error::concurrency_conflict(format!(
                "Slug {} already taken",
                slug_hash
            )));
        }

        slugs.insert(slug_hash.to_string(), (profile_id, region.clone()));
        profiles.insert(profile_id, region);
        Ok(())
    }

    async fn update_slug_routing(
        &self,
        profile_id: ProfileId,
        old_slug_hash: &str,
        new_slug_hash: &str,
        region: Region,
    ) -> Result<()> {
        let mut slugs = self.slugs.lock().unwrap();

        // Simulation de la LWT sur le nouveau slug
        if slugs.contains_key(new_slug_hash) {
            return Err(Error::concurrency_conflict(format!(
                "Slug {} already taken",
                new_slug_hash
            )));
        }

        slugs.remove(old_slug_hash);
        slugs.insert(new_slug_hash.to_string(), (profile_id, region));
        Ok(())
    }

    async fn delete_routing(&self, profile_id: ProfileId, slug_hash: &str) -> Result<()> {
        self.slugs.lock().unwrap().remove(slug_hash);
        self.profiles.lock().unwrap().remove(&profile_id);
        Ok(())
    }
}
