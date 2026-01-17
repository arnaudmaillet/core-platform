// crates/profile/src/application/use_cases/increment_post_count/mod.rs

use std::sync::Arc;
use shared_kernel::domain::events::AggregateRoot;
use shared_kernel::domain::entities::EntityOptionExt;
use shared_kernel::domain::repositories::OutboxRepository;
use shared_kernel::domain::transaction::TransactionManager;
use shared_kernel::errors::Result;
use shared_kernel::infrastructure::{TransactionManagerExt, with_retry, RetryConfig};
use crate::application::increment_post_count::IncrementPostCountCommand;
use crate::domain::repositories::ProfileRepository;

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
        Self { repo, outbox_repo, tx_manager }
    }

    pub async fn execute(&self, command: IncrementPostCountCommand) -> Result<()> {
        // RetryConfig court ici car les conflits sur les compteurs sont fréquents
        with_retry(RetryConfig::default(), || async {
            self.try_execute_once(&command).await
        }).await
    }

    async fn try_execute_once(&self, cmd: &IncrementPostCountCommand) -> Result<()> {
        // 1. Récupération (Identity-only pour la performance)
        let mut profile = self.repo.get_profile_identity(&cmd.account_id, &cmd.region)
            .await?
            .ok_or_not_found(cmd.account_id)?;

        // 2. Logique Métier
        profile.increment_post_count(cmd.post_id);

        // 3. Préparation pour la transaction
        let events = profile.pull_events();
        let p_cloned = profile.clone();

        // 4. Persistence Transactionnelle (Atomique)
        self.tx_manager.run_in_transaction(move |mut tx| {
            let repo = self.repo.clone();
            let outbox = self.outbox_repo.clone();
            let p = p_cloned.clone();
            let events_to_process = events;

            Box::pin(async move {
                repo.save(&p, Some(&mut *tx)).await?;

                // Enregistre les événements dans la table Outbox
                for event in events_to_process {
                    outbox.save(event.as_ref(), Some(&mut *tx)).await?;
                }
                Ok(())
            })
        }).await?;

        Ok(())
    }
}