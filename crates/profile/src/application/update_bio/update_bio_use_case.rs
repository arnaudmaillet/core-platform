// crates/profile/src/application/use_cases/update_bio/mod.rs

use std::sync::Arc;
use shared_kernel::domain::events::AggregateRoot;
use shared_kernel::domain::entities::EntityOptionExt;
use shared_kernel::domain::repositories::OutboxRepository;
use shared_kernel::domain::transaction::TransactionManager;
use shared_kernel::domain::utils::{with_retry, RetryConfig};
use shared_kernel::errors::Result;
use shared_kernel::infrastructure::postgres::transactions::TransactionManagerExt;
use crate::application::update_bio::UpdateBioCommand;
use crate::domain::entities::Profile;
use crate::domain::repositories::ProfileRepository;

pub struct UpdateBioUseCase {
    repo: Arc<dyn ProfileRepository>,
    outbox_repo: Arc<dyn OutboxRepository>,
    tx_manager: Arc<dyn TransactionManager>,
}

impl UpdateBioUseCase {
    pub fn new(
        repo: Arc<dyn ProfileRepository>,
        outbox_repo: Arc<dyn OutboxRepository>,
        tx_manager: Arc<dyn TransactionManager>,
    ) -> Self {
        Self { repo, outbox_repo, tx_manager }
    }

    pub async fn execute(&self, command: UpdateBioCommand) -> Result<Profile> {
        with_retry(RetryConfig::default(), || async {
            self.try_execute_once(&command).await
        }).await
    }

    async fn try_execute_once(&self, cmd: &UpdateBioCommand) -> Result<Profile> {
        // 1. Récupération du profil
        // En Hyperscale, on ne charge que l'identité + les champs nécessaires à la mutation
        let mut profile = self.repo.get_profile_by_account_id(&cmd.account_id, &cmd.region)
            .await?
            .ok_or_not_found(&cmd.account_id)?;

        // 2. Application du changement (Modèle Riche)
        if !profile.update_bio(cmd.new_bio.clone()) {
            return Ok(profile);
        }

        // 3. Extraction des événements
        let events = profile.pull_events();

        // Idempotence Applicative : Si aucun événement n'a été produit,
        // c'est que la donnée était identique. On s'arrête là (pas d'IO DB).
        if events.is_empty() {
            return Ok(profile);
        }

        let updated_profile = profile.clone();

        // 4. Persistence Transactionnelle
        self.tx_manager.run_in_transaction(move |mut tx| {
            let repo = self.repo.clone();
            let outbox = self.outbox_repo.clone();
            let profile = profile.clone();
            let events = events.clone();

            Box::pin(async move {
                repo.save(&profile, Some(&mut *tx)).await?;
                for event in events {
                    outbox.save(&mut *tx, event.as_ref()).await?;
                }

                Ok(())
            })
        }).await?;

        Ok(updated_profile)
    }
}