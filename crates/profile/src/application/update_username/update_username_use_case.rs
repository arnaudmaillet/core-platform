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
        let mut profile = self.repo
            .get_profile_by_account_id(&cmd.account_id, &cmd.region)
            .await?
            .ok_or_not_found(&cmd.account_id)?;

        // On prépare le changement
        if !profile.update_username(&cmd.region, cmd.new_username.clone())? {
            return Ok(profile);
        }

        // On extrait les événements
        let events = profile.pull_events();

        // On clone l'état FINAL pour la transaction
        let profile_to_persist = profile.clone();

        // On crée une copie dédiée à la closure pour laisser l'originale disponible pour le retour
        let profile_for_tx = profile_to_persist.clone();

        self.tx_manager
            .run_in_transaction(move |mut tx| {
                let repo = self.repo.clone();
                let outbox = self.outbox_repo.clone();
                let p = profile_for_tx.clone(); // On clone la copie à chaque essai de transaction
                let evs = events.clone();

                Box::pin(async move {
                    if repo.exists_by_username(&p.username(), &p.region_code()).await? {
                        return Err(DomainError::AlreadyExists {
                            entity: "Profile",
                            field: "username",
                            value: p.username().as_str().to_string(),
                        });
                    }

                    repo.save(&p, Some(&mut *tx)).await?;

                    for event in evs {
                        outbox.save(&mut *tx, event.as_ref()).await?;
                    }

                    tx.commit().await?;
                    Ok(())
                })
            })
            .await?;

        // On renvoie l'objet original (qui n'a pas été déplacé dans la closure)
        Ok(profile_to_persist)
    }
}
