// crates/profile/src/application/use_cases/update_social_links/mod.rs

use crate::application::use_cases::update_social_links::UpdateSocialLinksCommand;
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
        Self {
            repo,
            outbox_repo,
            tx_manager,
        }
    }

    pub async fn execute(&self, command: UpdateSocialLinksCommand) -> Result<Profile> {
        with_retry(RetryConfig::default(), || async {
            self.try_execute_once(&command).await
        })
        .await
    }

    async fn try_execute_once(&self, cmd: &UpdateSocialLinksCommand) -> Result<Profile> {
        // 1. Récupération du profil
        let original_profile = self
            .repo
            .assemble_full_profile(&cmd.profile_id, &cmd.region)
            .await?
            .ok_or_not_found(&cmd.profile_id)?;

        // 2. Application de la logique métier
        // L'entité vérifie si les liens ont changé, incrémente la version et émet l'événement
        let mut profile_to_update = original_profile.clone();
        
        if !profile_to_update.update_social_links(&cmd.region, cmd.new_links.clone())? {
            return Ok(original_profile)
        };

        // 3. Extraction des événements
        let events = profile_to_update.pull_events();
        let updated_profile = profile_to_update.clone();
        let repo = Arc::clone(&self.repo);
        let outbox = Arc::clone(&self.outbox_repo);

        // 4. Persistence Transactionnelle
        self.tx_manager
            .run_in_transaction(move |mut tx| {
                let repo = Arc::clone(&repo);
                let outbox = Arc::clone(&outbox);
                
                let original_for_tx = original_profile.clone();
                let updated_for_tx = profile_to_update.clone();
                let events_for_tx = events.clone();

                Box::pin(async move {
                    repo.save_identity(&updated_for_tx, Some(&original_for_tx), Some(&mut *tx)).await?;

                    for event in events_for_tx {
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
