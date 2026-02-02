// crates/profile/src/application/use_cases/create_profile/create_profile_use_case.rs

use std::sync::Arc;
use shared_kernel::domain::events::AggregateRoot;
use shared_kernel::domain::repositories::OutboxRepository;
use shared_kernel::domain::transaction::TransactionManager;
use shared_kernel::errors::{DomainError, Result};
use shared_kernel::domain::utils::{with_retry, RetryConfig};
use shared_kernel::infrastructure::postgres::transactions::TransactionManagerExt;
use crate::application::create_profile::CreateProfileCommand;
use crate::domain::builders::ProfileBuilder;
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

    pub async fn execute(&self, command: CreateProfileCommand) -> Result<Profile> {
        with_retry(RetryConfig::default(), || async {
            self.try_execute_once(&command).await
        }).await
    }

    async fn try_execute_once(&self, cmd: &CreateProfileCommand) -> Result<Profile> {
        // 1. Instanciation via le domaine
        let mut profile = Profile::create(
            ProfileBuilder::new(
                cmd.account_id.clone(),
                cmd.region.clone(),
                cmd.display_name.clone(),
                cmd.username.clone(),
            )
                .is_private(false)
                .build()
        );

        // 2. Extraction des événements et préparation de la donnée
        let events = profile.pull_events();
        let updated_profile = profile.clone();

        // 3. Exécution de la transaction atomique
        self.tx_manager.run_in_transaction(move |mut tx| {
            let repo = self.repo.clone();
            let outbox = self.outbox_repo.clone();
            let profile = profile.clone();
            let events = events.clone();

            Box::pin(async move {
                // Check d'unicité métier (avant insertion)
                if repo.exists_by_username(&profile.username(), &profile.region_code()).await? {
                    return Err(DomainError::AlreadyExists {
                        entity: "Profile",
                        field: "username",
                        value: profile.username().as_str().to_string(),
                    });
                }

                // Sauvegarde de l'agrégat (Version 1)
                repo.save(&profile, Some(&mut *tx)).await?;

                // Sauvegarde des événements (ProfileCreated, etc.)
                for event in events {
                    outbox.save(&mut *tx, event.as_ref()).await?;
                }

                Ok(())
            })
        }).await?;

        Ok(updated_profile)
    }
}