// crates/profile/src/application/use_cases/create_profile/create_profile_use_case.rs

use crate::application::create_profile::CreateProfileCommand;
use crate::domain::entities::Profile;
use crate::domain::repositories::ProfileRepository;
use shared_kernel::domain::events::AggregateRoot;
use shared_kernel::domain::repositories::OutboxRepository;
use shared_kernel::domain::transaction::TransactionManager;
use shared_kernel::domain::utils::{RetryConfig, with_retry};
use shared_kernel::errors::{DomainError, Result};
use shared_kernel::infrastructure::postgres::transactions::TransactionManagerExt;
use std::sync::Arc;

pub struct CreateProfileUseCase {
    repo: Arc<dyn ProfileRepository>,
    outbox_repo: Arc<dyn OutboxRepository>,
    tx_manager: Arc<dyn TransactionManager>,
}

impl CreateProfileUseCase {
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

    pub async fn execute(&self, command: CreateProfileCommand) -> Result<Profile> {
        with_retry(RetryConfig::default(), || async {
            self.try_execute_once(&command).await
        })
        .await
    }

    async fn try_execute_once(&self, cmd: &CreateProfileCommand) -> Result<Profile> {
        // 1. Instanciation via le domaine
        let mut profile = Profile::builder(
            cmd.account_id.clone(),
            cmd.region.clone(),
            cmd.display_name.clone(),
            cmd.username.clone(),
        )
            .with_privacy(false)
            .build();

        // 2. Extraction des événements et préparation de la donnée
        let events = profile.pull_events();

        if events.is_empty() {
            return Ok(profile);
        }

        let updated_profile = profile.clone();
        let profile_to_move = profile.clone();

        // 3. Exécution de la transaction atomique
        self.tx_manager
            .run_in_transaction(move |mut tx| {
                let repo = self.repo.clone();
                let outbox = self.outbox_repo.clone();
                let profile_for_tx = profile_to_move.clone();
                let events_for_tx = events.clone();

                Box::pin(async move {
                    // Check d'unicité (Source de vérité : Postgres via le Repo)
                    if repo.exists_by_username(&profile_for_tx.username(), &profile_for_tx.region_code()).await? {
                        return Err(DomainError::AlreadyExists {
                            entity: "Profile",
                            field: "username",
                            value: profile_for_tx.username().as_str().to_string(),
                        });
                    }

                    // Sauvegarde : On passe None pour 'original' car c'est une création
                    repo.save_identity(&profile_for_tx, None, Some(&mut *tx)).await?;

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
