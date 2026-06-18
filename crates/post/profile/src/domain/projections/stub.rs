use crate::ProjectedProfile;
use crate::domain::{ProfileReadProjection, ProfileWriteProjection};
use async_trait::async_trait;
use shared_kernel::core::Result;
use shared_kernel::types::ProfileId;
use std::collections::HashMap;
use std::sync::RwLock;

pub struct ProfileProjectionStub {
    profiles: RwLock<HashMap<ProfileId, ProjectedProfile>>,
}

impl ProfileProjectionStub {
    pub fn new() -> Self {
        Self {
            profiles: RwLock::new(HashMap::new()),
        }
    }

    /// Helper de test pour pré-alimenter le stub (Seed data / Fixtures)
    /// sans passer par le trait d'écriture (évite de devoir générer un timestamp)
    pub fn feed(&self, profile: ProjectedProfile) {
        let mut profiles = self.profiles.write().unwrap();
        profiles.insert(profile.id.clone(), profile);
    }

    /// Helper pour forcer la création rapide d'un profil actif standard en test
    pub fn simulate_active_profile(&self, profile_id: ProfileId) {
        let profile = ProjectedProfile {
            id: profile_id.clone(),
            handle: format!("user_{}", profile_id),
            display_name: "John Doe".to_string(),
            avatar_url: Some("https://cdn.wynn.tv/assets/default-avatar.png".to_string()),
            is_verified: false,
        };
        self.feed(profile);
    }

    /// Helper d'assertion : Vérifie si un profil existe dans la mémoire du stub
    pub fn contains(&self, profile_id: &ProfileId) -> bool {
        self.profiles.read().unwrap().contains_key(profile_id)
    }
}

// --- Implémentation du contrat de LECTURE ---
#[async_trait]
impl ProfileReadProjection for ProfileProjectionStub {
    async fn find_by_id(&self, profile_id: &ProfileId) -> Result<Option<ProjectedProfile>> {
        let profiles = self.profiles.read().unwrap();
        Ok(profiles.get(profile_id).cloned())
    }
}

// --- Implémentation du contrat d'ÉCRITURE ---
#[async_trait]
impl ProfileWriteProjection for ProfileProjectionStub {
    async fn save(&self, profile: &ProjectedProfile, _updated_at_ms: i64) -> Result<()> {
        let mut profiles = self.profiles.write().unwrap();
        profiles.insert(profile.id.clone(), profile.clone());
        Ok(())
    }
}
