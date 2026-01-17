// crates/profile/src/application/use_cases/update_social_links/mod.rs

use std::sync::Arc;
use shared_kernel::domain::events::AggregateRoot;
use shared_kernel::domain::entities::EntityOptionExt;
use shared_kernel::domain::repositories::OutboxRepository;
use shared_kernel::domain::transaction::TransactionManager;
use shared_kernel::errors::Result;
use shared_kernel::infrastructure::{TransactionManagerExt, with_retry, RetryConfig};
use crate::application::update_social_links::UpdateSocialLinksCommand;
use crate::domain::repositories::ProfileRepository;

pub struct UpdateSocialLinksUseCase {
    repo: Arc<dyn ProfileRepository>,
    outbox_repo: Arc<dyn OutboxRepository>,
    tx_manager: Arc<dyn TransactionManager>,
}

impl UpdateSocialLinksUseCase {
    pub fn new(
        repo: Arc<dyn ProfileRepository>,
        outbox_repo: Arc<dyn OutboxRepository>,
        tx_manager: Arc<dyn TransactionManager>,
    ) -> Self {
        Self { repo, outbox_repo, tx_manager }
    }

    pub async fn execute(&self, command: UpdateSocialLinksCommand) -> Result<()> {
        with_retry(RetryConfig::default(), || async {
            self.try_execute_once(&command).await
        }).await
    }

    async fn try_execute_once(&self, cmd: &UpdateSocialLinksCommand) -> Result<()> {
        // 1. Récupération du profil
        let mut profile = self.repo.get_profile_identity(&cmd.account_id, &cmd.region)
            .await?
            .ok_or_not_found(cmd.account_id)?;

        // 2. Application de la logique métier
        // L'entité vérifie si les liens ont changé, incrémente la version et émet l'événement
        profile.update_social_links(cmd.links.clone());

        // 3. Extraction des événements
        let events = profile.pull_events();
        let p_cloned = profile.clone();

        // 4. Persistence Transactionnelle
        self.tx_manager.run_in_transaction(move |mut tx| {
            let repo = self.repo.clone();
            let outbox = self.outbox_repo.clone();
            let p = p_cloned.clone();
            let events_to_process = events;

            Box::pin(async move {
                repo.save(&p, Some(&mut *tx)).await?;

                for event in events_to_process {
                    outbox.save(event.as_ref(), Some(&mut *tx)).await?;
                }
                Ok(())
            })
        }).await?;

        Ok(())
    }
}