// crates/social/src/application/context/query.rs

use std::sync::Arc;

use crate::application::context::SocialKernelCtx;
use crate::domain::entities::ProfileCounters;
use crate::repositories::ProfileCountersStorageRepository;
use shared_kernel::core::ErrorCode;
use shared_kernel::types::Counter;
use shared_kernel::{
    core::{Error, Result},
    types::{ProfileId, Region},
};

#[derive(Clone)]
pub struct SocialQueryCtx {
    kernel: SocialKernelCtx,
    profile_counters_storage: Arc<dyn ProfileCountersStorageRepository>,
    region: Region,
}

impl SocialQueryCtx {
    pub fn new(
        kernel: SocialKernelCtx,
        profile_counters_storage: Arc<dyn ProfileCountersStorageRepository>,
        region: Region,
    ) -> Self {
        Self {
            kernel,
            profile_counters_storage,
            region,
        }
    }

    pub fn region(&self) -> Region {
        self.region
    }

    pub async fn is_already_following(
        &self,
        follower_id: ProfileId,
        following_id: ProfileId,
    ) -> Result<bool> {
        self.kernel
            .follow_relation_repo()
            .is_following(follower_id, following_id)
            .await
    }

    pub async fn get_following_list(
        &self,
        follower_id: ProfileId,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<ProfileId>> {
        self.kernel
            .follow_relation_repo()
            .get_following_ids(follower_id, limit, offset)
            .await
    }

    pub async fn get_followers_list(
        &self,
        following_id: ProfileId,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<ProfileId>> {
        self.kernel
            .follow_relation_repo()
            .get_followers_ids(following_id, limit, offset)
            .await
    }

    pub async fn get_profile_counters(&self, profile_id: ProfileId) -> Result<ProfileCounters> {
        // 1. Tentative de lecture sur l'index à chaud (Redis)
        match self.kernel.profile_counters_index().read(profile_id).await {
            Ok(counters) => Ok(counters),

            // 2. Cache Miss ! L'information n'est pas dans Redis
            Err(Error {
                code: ErrorCode::NotFound,
                ..
            }) => {
                // On interroge ScyllaDB (fetch retourne un Option<ProfileCounters>)
                let db_counters_opt = self.profile_counters_storage.fetch(profile_id).await?;

                // Si le profil n'existe pas non plus en base, on applique une structure par défaut
                let db_counters = db_counters_opt.unwrap_or_else(|| {
                    ProfileCounters::restore(
                        profile_id,
                        Counter::default(),
                        Counter::default(),
                        chrono::Utc::now(),
                    )
                });

                // 3. Réchauffement asynchrone du cache Redis pour les prochaines requêtes
                if let Err(e) = self
                    .kernel
                    .profile_counters_index()
                    .save(&db_counters)
                    .await
                {
                    tracing::warn!(
                        "Failed to warm up Redis counter cache for {}: {:?}",
                        profile_id,
                        e
                    );
                }

                Ok(db_counters)
            }

            Err(other_error) => Err(other_error),
        }
    }
}
