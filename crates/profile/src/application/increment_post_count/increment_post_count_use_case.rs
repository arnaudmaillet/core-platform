// crates/profile/src/application/use_cases/increment_post_count/mod.rs

use crate::application::increment_post_count::IncrementPostCountCommand;
use crate::domain::entities::Profile;
use crate::domain::repositories::ProfileRepository;
use shared_kernel::domain::entities::EntityOptionExt;
use shared_kernel::domain::events::AggregateRoot;
use shared_kernel::domain::repositories::OutboxRepository;
use shared_kernel::domain::transaction::TransactionManager;
use shared_kernel::domain::utils::{RetryConfig, with_retry};
use shared_kernel::errors::Result;
use shared_kernel::infrastructure::postgres::transactions::TransactionManagerExt;
use std::sync::Arc;

pub struct IncrementPostCountUseCase {
    repo: Arc<dyn ProfileRepository>,
    outbox_repo: Arc<dyn OutboxRepository>,
    tx_manager: Arc<dyn TransactionManager>,
}

impl IncrementPostCountUseCase {
    pub fn new(
        repo: Arc<dyn ProfileRepository>,
        outbox_repo: Arc<dyn OutboxRepository>,
        tx_manager: Arc<dyn TransactionManager>,
    ) -> Self {
        Self {
            repo,
            outbox_repo,
            tx_manager,
        }
    }

    pub async fn execute(&self, command: IncrementPostCountCommand) -> Result<Profile> {
        // RetryConfig court ici car les conflits sur les compteurs sont fréquents
        with_retry(RetryConfig::default(), || async {
            self.try_execute_once(&command).await
        })
        .await
    }

    async fn try_execute_once(&self, cmd: &IncrementPostCountCommand) -> Result<Profile> {
        // 1. Récupération (Identity-only pour la performance)
        let mut profile = self
            .repo
            .get_profile_by_account_id(&cmd.account_id, &cmd.region)
            .await?
            .ok_or_not_found(&cmd.account_id)?;

        // 2. Logique Métier
        profile.increment_post_count(cmd.post_id);

        // 3. Préparation pour la transaction
        let events = profile.pull_events();

        if events.is_empty() {
            return Ok(profile);
        }

        let updated_profile = profile.clone();

        // 4. Persistence Transactionnelle (Atomique)
        self.tx_manager
            .run_in_transaction(move |mut tx| {
                let repo = self.repo.clone();
                let outbox = self.outbox_repo.clone();
                let profile = profile.clone();
                let events = events.clone();

                Box::pin(async move {
                    repo.save(&profile, Some(&mut *tx)).await?;

                    // Enregistre les événements dans la table Outbox
                    for event in events {
                        outbox.save(&mut *tx, event.as_ref()).await?;
                    }
                    tx.commit().await?;
                    Ok(())
                })
            })
            .await?;

        Ok(updated_profile)
    }
}
