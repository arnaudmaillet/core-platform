// crates/profile/src/application/use_cases/update_stats/mod.rs

use std::sync::Arc;
use shared_kernel::errors::Result;
use shared_kernel::domain::utils::{with_retry, RetryConfig};
use crate::application::update_stats::UpdateStatsCommand;
use crate::domain::repositories::ProfileStatsRepository;

pub struct UpdateStatsUseCase {
    profile_repo: Arc<dyn ProfileStatsRepository>,
}

impl UpdateStatsUseCase {
    pub fn new(profile_repo: Arc<dyn ProfileStatsRepository>) -> Self {
        Self { profile_repo }
    }

    pub async fn execute(&self, command: UpdateStatsCommand) -> Result<()> {
        // Pour ScyllaDB, le retry est géré au niveau driver,
        // mais on garde une sécurité ici pour la logique d'application.
        with_retry(RetryConfig::default(), || async {
            self.try_execute_once(&command).await
        }).await
    }

    async fn try_execute_once(&self, cmd: &UpdateStatsCommand) -> Result<()> {
        // 1. Mise à jour atomique dans ScyllaDB
        // On envoie directement les deltas (follower_count = follower_count + delta)
        self.profile_repo.update_stats(
            &cmd.account_id,
            &cmd.region,
            cmd.follower_delta,
            cmd.following_delta,
            cmd.post_delta
        ).await?;

        // Note: Ici, on ne passe pas par l'Outbox Postgres car ScyllaDB
        // est conçu pour la disponibilité. Si on veut synchroniser ailleurs,
        // on utilisera un CDC (Change Data Capture) sur ScyllaDB.

        Ok(())
    }
}