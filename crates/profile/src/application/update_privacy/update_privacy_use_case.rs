// crates/profile/src/application/use_cases/update_privacy/mod.rs

use crate::application::update_privacy::UpdatePrivacyCommand;
use crate::domain::entities::Profile;
use crate::domain::events::ProfileEvent;
use crate::domain::repositories::ProfileRepository;
use shared_kernel::domain::entities::EntityOptionExt;
use shared_kernel::domain::events::AggregateRoot;
use shared_kernel::domain::repositories::OutboxRepository;
use shared_kernel::domain::transaction::TransactionManager;
use shared_kernel::domain::utils::{RetryConfig, with_retry};
use shared_kernel::errors::Result;
use shared_kernel::infrastructure::postgres::transactions::TransactionManagerExt;
use std::sync::Arc;

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
        Self {
            repo,
            outbox_repo,
            tx_manager,
        }
    }

    pub async fn execute(&self, command: UpdatePrivacyCommand) -> Result<Profile> {
        with_retry(RetryConfig::default(), || async {
            self.try_execute_once(&command).await
        })
        .await
    }

    async fn try_execute_once(&self, cmd: &UpdatePrivacyCommand) -> Result<Profile> {
        // 1. Récupération du profil
        let original_profile = self
            .repo
            .assemble_full_profile(&cmd.profile_id, &cmd.region)
            .await?
            .ok_or_not_found(&cmd.profile_id)?;

        let mut profile_to_update = original_profile.clone();
        
        if !profile_to_update.update_privacy(&cmd.region, cmd.is_private)? {
            return Ok(original_profile);
        };

        // 4. Extraction et Persistence
        let events = profile_to_update.pull_events();

        if events.is_empty() {
            return Ok(profile_to_update);
        }
        
        let updated_profile = profile_to_update.clone();

        self.tx_manager
            .run_in_transaction(move |mut tx| {
                let repo = self.repo.clone();
                let outbox = self.outbox_repo.clone();

                let original_for_tx = original_profile.clone();
                let updated_for_tx = profile_to_update.clone();
                let events_for_tx = events.clone();

                Box::pin(async move {
                    // Sauvegarde l'état (avec vérification de version)
                    repo.save_identity(&updated_for_tx, Some(&original_for_tx), Some(&mut *tx)).await?;

                    // Sauvegarde de l'événement Outbox (Crucial pour le Feed et Search)
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
