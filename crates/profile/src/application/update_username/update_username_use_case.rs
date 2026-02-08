use crate::application::update_username::UpdateUsernameCommand;
use crate::domain::entities::Profile;
use crate::domain::repositories::ProfileRepository;
use shared_kernel::domain::entities::EntityOptionExt;
use shared_kernel::domain::events::AggregateRoot;
use shared_kernel::domain::repositories::OutboxRepository;
use shared_kernel::domain::transaction::TransactionManager;
use shared_kernel::domain::utils::{RetryConfig, with_retry};
use shared_kernel::errors::{DomainError, Result};
use shared_kernel::infrastructure::postgres::transactions::TransactionManagerExt;
use std::sync::Arc;

pub struct UpdateUsernameUseCase {
    repo: Arc<dyn ProfileRepository>,
    outbox_repo: Arc<dyn OutboxRepository>,
    tx_manager: Arc<dyn TransactionManager>,
}

impl UpdateUsernameUseCase {
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

    /// Exécution avec stratégie de retry pour gérer les conflits de concurrence (OCC)
    pub async fn execute(&self, command: UpdateUsernameCommand) -> Result<Profile> {
        with_retry(RetryConfig::default(), || async {
            self.try_execute_once(&command).await
        })
        .await
    }

    async fn try_execute_once(&self, cmd: &UpdateUsernameCommand) -> Result<Profile> {
        let original_profile = self.repo
            .assemble_full_profile(&cmd.account_id, &cmd.region)
            .await?
            .ok_or_not_found(&cmd.account_id)?;

        // On prépare le changement
        let mut profile_to_update = original_profile.clone();
        
        if !profile_to_update.update_username(&cmd.region, cmd.new_username.clone())? {
            return Ok(original_profile);
        }

        // On extrait les événements
        let events = profile_to_update.pull_events();

        if events.is_empty() {
            return Ok(profile_to_update);
        }

        // On clone l'état FINAL pour la transaction
        let updated_profile = profile_to_update.clone();

        self.tx_manager
            .run_in_transaction(move |mut tx| {
                let repo = self.repo.clone();
                let outbox = self.outbox_repo.clone();

                let original_for_tx = original_profile.clone();
                let updated_for_tx = profile_to_update.clone();
                let events_for_tx = events.clone();
                
                Box::pin(async move {
                    if repo.exists_by_username(&updated_for_tx.username(), &updated_for_tx.region_code()).await? {
                        return Err(DomainError::AlreadyExists {
                            entity: "Profile",
                            field: "username",
                            value: updated_for_tx.username().as_str().to_string(),
                        });
                    }

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
