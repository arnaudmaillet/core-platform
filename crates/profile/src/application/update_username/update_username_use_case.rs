use std::sync::Arc;
use shared_kernel::domain::events::AggregateRoot;
use shared_kernel::domain::entities::EntityOptionExt;
use shared_kernel::domain::repositories::OutboxRepository;
use shared_kernel::domain::transaction::TransactionManager;
use shared_kernel::domain::utils::{with_retry, RetryConfig};
use shared_kernel::errors::{Result, DomainError};
use shared_kernel::infrastructure::postgres::transactions::TransactionManagerExt;
use crate::application::update_username::UpdateUsernameCommand;
use crate::domain::entities::Profile;
use crate::domain::repositories::ProfileRepository;

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
        Self { repo, outbox_repo, tx_manager }
    }

    /// Exécution avec stratégie de retry pour gérer les conflits de concurrence (OCC)
    pub async fn execute(&self, command: UpdateUsernameCommand) -> Result<Profile> {
        with_retry(RetryConfig::default(), || async {
            self.try_execute_once(&command).await
        }).await
    }

    async fn try_execute_once(&self, cmd: &UpdateUsernameCommand) -> Result<Profile> {
        // 1. Validation syntaxique du nouveau slug (via Value Object)
        let new_username = cmd.new_username.clone();

        // 2. Récupération du profil existant
        let mut profile = self.repo.get_profile_by_account_id(&cmd.account_id, &cmd.region)
            .await?
            .ok_or_not_found(&cmd.account_id)?;

        if !profile.update_username(new_username) {
            return Ok(profile);
        }

        // 5. Extraction des faits (événements) et préparation de la persistence
        let events = profile.pull_events();
        let updated_profile = profile.clone();

        // 6. Phase de commit transactionnel (Garantie Hyperscale)
        self.tx_manager.run_in_transaction(|mut tx| {
            let repo = self.repo.clone();
            let outbox = self.outbox_repo.clone();
            let p = updated_profile.clone();
            let events_to_process = events;

            Box::pin(async move {
                // PROTECTION CRITIQUE : Double vérification d'unicité à l'intérieur de la transaction
                // Empêche deux utilisateurs de prendre le même slug simultanément
                if repo.exists_by_username(&p.username, &p.region_code).await? {
                    return Err(DomainError::AlreadyExists {
                        entity: "Profile",
                        field: "username_slug",
                        value: p.username.as_str().to_string(),
                    });
                }

                // Persistence de l'entité (Postgres)
                // Le repo doit injecter le WHERE version = current_version
                repo.save(&p, Some(&mut *tx)).await?;

                // Persistence des événements (Outbox)
                // Permet au service de redirection et au moteur de recherche de réagir
                for event in events_to_process {
                    outbox.save(&mut *tx, event.as_ref()).await?;
                }

                Ok(())
            })
        }).await?;

        Ok(updated_profile)
    }
}