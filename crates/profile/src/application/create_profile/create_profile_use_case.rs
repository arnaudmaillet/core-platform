// crates/profile/src/application/use_cases/create_profile/create_profile_use_case.rs

use std::sync::Arc;
use shared_kernel::domain::events::AggregateRoot;
use shared_kernel::domain::repositories::OutboxRepository;
use shared_kernel::domain::transaction::TransactionManager;
use shared_kernel::errors::{DomainError, Result};
use shared_kernel::infrastructure::{with_retry, RetryConfig, TransactionManagerExt};
use crate::application::create_profile::CreateProfileCommand;
use crate::domain::entities::Profile;
use crate::domain::repositories::ProfileRepository;

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
        Self { repo, outbox_repo, tx_manager }
    }

    pub async fn execute(&self, command: CreateProfileCommand) -> Result<()> {
        with_retry(RetryConfig::default(), || async {
            self.try_execute_once(&command).await
        }).await
    }

    async fn try_execute_once(&self, cmd: &CreateProfileCommand) -> Result<()> {
        // 1. Instanciation via le domaine
        let mut profile = Profile::new_initial(
            cmd.account_id.clone(),
            cmd.region.clone(),
            cmd.display_name.clone(),
            cmd.username.clone(),
        );

        // 2. Extraction des événements et préparation de la donnée
        let events = profile.pull_events();
        let p_cloned = profile.clone();

        // 3. Exécution de la transaction atomique
        self.tx_manager.run_in_transaction(move |mut tx| {
            let repo = self.repo.clone();
            let outbox = self.outbox_repo.clone();
            let p = p_cloned.clone();
            let events_to_save = events;

            Box::pin(async move {
                // Check d'unicité métier (avant insertion)
                if repo.exists_by_username(&p.username, &p.region_code).await? {
                    return Err(DomainError::AlreadyExists {
                        entity: "Profile",
                        field: "username",
                        value: p.username.as_str().to_string(),
                    });
                }

                // Sauvegarde de l'agrégat (Version 1)
                repo.save(&p, Some(&mut *tx)).await?;

                // Sauvegarde des événements (ProfileCreated, etc.)
                for event in events_to_save {
                    outbox.save(event.as_ref(), Some(&mut *tx)).await?;
                }

                Ok(())
            })
        }).await?;

        Ok(())
    }
}