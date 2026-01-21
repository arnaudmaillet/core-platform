// crates/profile/src/application/use_cases/update_privacy/mod.rs

use std::sync::Arc;
use shared_kernel::domain::events::AggregateRoot;
use shared_kernel::domain::entities::EntityOptionExt;
use shared_kernel::domain::repositories::OutboxRepository;
use shared_kernel::domain::transaction::TransactionManager;
use shared_kernel::errors::Result;
use shared_kernel::domain::utils::{with_retry, RetryConfig};
use shared_kernel::infrastructure::postgres::transactions::TransactionManagerExt;
use crate::application::update_privacy::UpdatePrivacyCommand;
use crate::domain::repositories::ProfileRepository;
use crate::domain::events::ProfileEvent;

pub struct UpdatePrivacyUseCase {
    repo: Arc<dyn ProfileRepository>,
    outbox_repo: Arc<dyn OutboxRepository>,
    tx_manager: Arc<dyn TransactionManager>,
}

impl UpdatePrivacyUseCase {
    pub fn new(
        repo: Arc<dyn ProfileRepository>,
        outbox_repo: Arc<dyn OutboxRepository>,
        tx_manager: Arc<dyn TransactionManager>,
    ) -> Self {
        Self { repo, outbox_repo, tx_manager }
    }

    pub async fn execute(&self, command: UpdatePrivacyCommand) -> Result<()> {
        with_retry(RetryConfig::default(), || async {
            self.try_execute_once(&command).await
        }).await
    }

    async fn try_execute_once(&self, cmd: &UpdatePrivacyCommand) -> Result<()> {
        // 1. Récupération du profil
        let mut profile = self.repo.get_profile_without_stats(&cmd.account_id, &cmd.region)
            .await?
            .ok_or_not_found(cmd.account_id)?;

        if !profile.update_privacy(cmd.is_private){
            return Ok(())
        };

        // 4. Extraction et Persistence
        let events = profile.pull_events();
        let p_cloned = profile.clone();

        self.tx_manager.run_in_transaction(move |mut tx| {
            let repo = self.repo.clone();
            let outbox = self.outbox_repo.clone();
            let p = p_cloned.clone();
            let events_to_process = events;

            Box::pin(async move {
                // Sauvegarde l'état (avec vérification de version)
                repo.save(&p, Some(&mut *tx)).await?;

                // Sauvegarde de l'événement Outbox (Crucial pour le Feed et Search)
                for event in events_to_process {
                    outbox.save(&mut *tx, event.as_ref()).await?;
                }

                Ok(())
            })
        }).await?;

        Ok(())
    }
}