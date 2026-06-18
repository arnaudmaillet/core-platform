// crates/post/src/application/services/profile_projection_orchestrator.rs

use crate::application::cache::ProfileCacheRepository;
use crate::application::repositories::ProfileProjectionRepository;
use shared_kernel::core::{Error, Result};
use shared_kernel::types::{ProfileId, Region};
use shared_proto::profile::v1::ProfileSummaryDto;
use std::sync::Arc;

pub struct ProfileProjectionOrchestrator {
    profile_repo: Arc<dyn ProfileProjectionRepository>,
    profile_cache_repo: Arc<dyn ProfileCacheRepository>,
}

impl ProfileProjectionOrchestrator {
    pub fn new(
        profile_repo: Arc<dyn ProfileProjectionRepository>,
        profile_cache_repo: Arc<dyn ProfileCacheRepository>,
    ) -> Self {
        Self {
            profile_repo,
            profile_cache_repo,
        }
    }

    /// Consomme l'événement de modification de profil
    /// pour mettre à jour la réplication/projection locale au service Post.
    pub async fn project_change(
        &self,
        region: Region,
        profile: ProfileSummaryDto,
        updated_at_ms: i64,
    ) -> Result<()> {
        // 1. Validation et typage fort de l'identifiant
        let profile_id = ProfileId::try_new(&profile.profile_id).map_err(|e| {
            Error::validation(
                "profile_id",
                format!("Invalid ProfileId in projection event: {}", e),
            )
        })?;

        // 2. ÉVICTION DU CACHE (Principe défensif indispensable)
        // On vire immédiatement l'ancienne version compacte de Redis.
        // Si une lecture (GetPost) survient une milliseconde après, elle fera un cache-miss
        // et lira la donnée fraîche de ScyllaDB ou attendra la fin de l'écriture.
        self.profile_cache_repo.invalidate(&profile_id).await?;

        // 3. PERSISTANCE DANS SCYLLADB
        // On applique l'écriture locale avec la sécurité du USING TIMESTAMP (updated_at_ms)
        // fournie par l'événement d'origine de l'Identity/Profile service.
        // Cela garantit l'idempotence des out-of-order events de Kafka.
        self.profile_repo.save(&profile, updated_at_ms).await?;

        tracing::info!(
            %profile_id,
            ?region,
            "Profile projection state successfully synchronized and cache evicted"
        );

        Ok(())
    }
}
