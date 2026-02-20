// crates/profile/src/application/get_profile_by_username/get_profile_by_username_use_case

use crate::domain::entities::Profile;
use crate::domain::repositories::ProfileRepository;
use shared_kernel::domain::repositories::CacheRepository;
use shared_kernel::errors::{DomainError, Result};
use shared_kernel::infrastructure::concurrency::Singleflight;
use std::sync::Arc;
use crate::application::use_cases::get_profile_by_handle::GetProfileByHandleCommand;

pub struct GetProfileByHandleUseCase {
    repo: Arc<dyn ProfileRepository>,
    cache: Arc<dyn CacheRepository>,
    sf: Singleflight<String, Profile>,
}

impl GetProfileByHandleUseCase {
    pub fn new(repo: Arc<dyn ProfileRepository>, cache: Arc<dyn CacheRepository>) -> Self {
        Self {
            repo,
            cache,
            sf: Singleflight::new(),
        }
    }

    pub async fn execute(&self, cmd: GetProfileByHandleCommand) -> Result<Profile> {
        let cache_key = format!(
            "profile:h:{}:{}",
            cmd.region.as_str(),
            cmd.handle.as_str()
        );

        // 1. TENTATIVE CACHE (Fast Path)
        // On récupère la String, puis on tente de la transformer en Profile
        if let Ok(Some(cached_json)) = self.cache.get(&cache_key).await {
            if let Ok(profile) = serde_json::from_str::<Profile>(&cached_json) {
                return Ok(profile);
            }
            // Si le JSON est corrompu, on continue pour rafraîchir le cache
        }

        // 2. PROTECTION SINGLEFLIGHT
        let sf_key = cache_key.clone();
        let profile = self
            .sf
            .execute(sf_key, || {
                let repo = Arc::clone(&self.repo);
                let cache = Arc::clone(&self.cache);
                let handle = cmd.handle.clone();
                let region = cmd.region.clone();
                let key = cache_key.clone();

                async move {
                    let p = repo
                        .resolve_profile_from_handle(&handle, &region)
                        .await?
                        .ok_or_else(|| DomainError::NotFound {
                            entity: "Profile",
                            id: handle.as_str().to_string(),
                        })?;

                    // On sérialise en JSON avant de stocker dans Redis
                    if let Ok(json) = serde_json::to_string(&p) {
                        let _ = cache
                            .set(&key, &json, Some(std::time::Duration::from_secs(3600)))
                            .await;
                    }

                    Ok(p)
                }
            })
            .await?;

        Ok(profile)
    }
}
